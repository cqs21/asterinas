// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::{AtomicBool, Ordering};

use aster_bigtcp::{
    socket::{NeedIfacePoll, RawTcpOption, RawTcpSetOption},
    wire::IpEndpoint,
};
use connected::ConnectedStream;
use connecting::{ConnResult, ConnectingStream};
use init::InitStream;
use listen::ListenStream;
use options::{
    Congestion, DeferAccept, Inq, KeepIdle, MaxSegment, NoDelay, SynCnt, UserTimeout, WindowClamp,
    KEEPALIVE_INTERVAL,
};
use ostd::sync::{PreemptDisabled, RwLockReadGuard, RwLockWriteGuard};
use takeable::Takeable;
use util::{Retrans, TcpOptionSet};

use super::UNSPECIFIED_LOCAL_ENDPOINT;
use crate::{
    events::IoEvents,
    fs::file_handle::FileLike,
    match_sock_option_mut, match_sock_option_ref,
    net::{
        iface::Iface,
        socket::{
            options::{Error as SocketError, SocketOption},
            private::SocketPrivate,
            util::{
                options::{SetSocketLevelOption, SocketOptionSet},
                send_recv_flags::SendRecvFlags,
                shutdown_cmd::SockShutdownCmd,
                socket_addr::SocketAddr,
                MessageHeader,
            },
            Socket,
        },
    },
    prelude::*,
    process::signal::{PollHandle, Pollable, Pollee},
    util::{MultiRead, MultiWrite},
};

mod connected;
mod connecting;
mod init;
mod listen;
mod observer;
pub mod options;
mod util;

pub(in crate::net) use self::observer::StreamObserver;
pub use self::util::CongestionControl;

pub struct StreamSocket {
    options: RwLock<OptionSet>,
    state: RwLock<Takeable<State>, PreemptDisabled>,
    is_nonblocking: AtomicBool,
    pollee: Pollee,
}

enum State {
    // Start state
    Init(InitStream),
    // Intermediate state
    Connecting(ConnectingStream),
    // Final State 1
    Connected(ConnectedStream),
    // Final State 2
    Listen(ListenStream),
}

#[derive(Debug, Clone)]
struct OptionSet {
    socket: SocketOptionSet,
    tcp: TcpOptionSet,
}

impl OptionSet {
    fn new() -> Self {
        let socket = SocketOptionSet::new_tcp();
        let tcp = TcpOptionSet::new();
        OptionSet { socket, tcp }
    }

    fn raw(&self) -> RawTcpOption {
        RawTcpOption {
            keep_alive: self.socket.keep_alive().then_some(KEEPALIVE_INTERVAL),
            is_nagle_enabled: !self.tcp.no_delay(),
        }
    }
}

impl StreamSocket {
    pub fn new(is_nonblocking: bool) -> Arc<Self> {
        let init_stream = InitStream::new();
        Arc::new(Self {
            options: RwLock::new(OptionSet::new()),
            state: RwLock::new(Takeable::new(State::Init(init_stream))),
            is_nonblocking: AtomicBool::new(is_nonblocking),
            pollee: Pollee::new(),
        })
    }

    fn new_accepted(connected_stream: ConnectedStream) -> Arc<Self> {
        let options = connected_stream.raw_with(|raw_tcp_socket| {
            let mut options = OptionSet::new();

            if raw_tcp_socket.keep_alive().is_some() {
                options.socket.set_keep_alive(true);
            }

            if !raw_tcp_socket.nagle_enabled() {
                options.tcp.set_no_delay(true);
            }

            // TODO: Update other options for a newly-accepted socket

            options
        });

        let pollee = Pollee::new();
        connected_stream.init_observer(StreamObserver::new(pollee.clone()));

        Arc::new(Self {
            options: RwLock::new(options),
            state: RwLock::new(Takeable::new(State::Connected(connected_stream))),
            is_nonblocking: AtomicBool::new(false),
            pollee,
        })
    }

    /// Ensures that the socket state is up to date and obtains a read lock on it.
    ///
    /// For a description of what "up-to-date" means, see [`Self::update_connecting`].
    fn read_updated_state(&self) -> RwLockReadGuard<Takeable<State>, PreemptDisabled> {
        loop {
            let state = self.state.read();
            match state.as_ref() {
                State::Connecting(connecting_stream) if connecting_stream.has_result() => (),
                _ => return state,
            };
            drop(state);

            self.update_connecting();
        }
    }

    /// Ensures that the socket state is up to date and obtains a write lock on it.
    ///
    /// For a description of what "up-to-date" means, see [`Self::update_connecting`].
    fn write_updated_state(&self) -> RwLockWriteGuard<Takeable<State>, PreemptDisabled> {
        self.update_connecting().1
    }

    /// Updates the socket state if the socket is an obsolete connecting socket.
    ///
    /// A connecting socket can become obsolete because some network events can set the socket to
    /// connected state (if the connection succeeds) or initial state (if the connection is
    /// refused) in [`Self::update_io_events`], but the state transition is delayed until the user
    /// operates on the socket to avoid too many locks in the interrupt handler.
    ///
    /// This method performs the delayed state transition to ensure that the state is up to date
    /// and returns the guards of the write-locked options and state.
    fn update_connecting(
        &self,
    ) -> (
        RwLockWriteGuard<OptionSet, PreemptDisabled>,
        RwLockWriteGuard<Takeable<State>, PreemptDisabled>,
    ) {
        // Hold the lock in advance to avoid race conditions.
        let mut options = self.options.write();
        let mut state = self.state.write();

        match state.as_ref() {
            State::Connecting(connection_stream) if connection_stream.has_result() => (),
            _ => return (options, state),
        }

        state.borrow(|owned_state| {
            let State::Connecting(connecting_stream) = owned_state else {
                unreachable!("`State::Connecting` is checked before calling `borrow_result`");
            };

            match connecting_stream.into_result() {
                ConnResult::Connecting(connecting_stream) => State::Connecting(connecting_stream),
                ConnResult::Connected(connected_stream) => {
                    options.socket.set_sock_errors(None);
                    State::Connected(connected_stream)
                }
                ConnResult::Refused(init_stream) => {
                    options.socket.set_sock_errors(Some(Error::with_message(
                        Errno::ECONNREFUSED,
                        "the connection is refused",
                    )));
                    State::Init(init_stream)
                }
            }
        });

        (options, state)
    }

    // Returns `None` to block the task and wait for the connection to be established, and returns
    // `Some(_)` if blocking is not necessary or not allowed.
    fn start_connect(&self, remote_endpoint: &IpEndpoint) -> Option<Result<()>> {
        let is_nonblocking = self.is_nonblocking();
        let (mut options, mut state) = self.update_connecting();

        let raw_option = options.raw();

        let (result_or_block, iface_to_poll) = state.borrow_result(|mut owned_state| {
            let init_stream = match owned_state {
                State::Init(init_stream) => init_stream,
                State::Connecting(_) if is_nonblocking => {
                    return (
                        owned_state,
                        (
                            Some(Err(Error::with_message(
                                Errno::EALREADY,
                                "the socket is connecting",
                            ))),
                            None,
                        ),
                    );
                }
                State::Connecting(_) => return (owned_state, (None, None)),
                State::Connected(ref mut connected_stream) => {
                    let err = connected_stream.check_new();
                    return (owned_state, (Some(err), None));
                }
                State::Listen(_) => {
                    return (
                        owned_state,
                        (
                            Some(Err(Error::with_message(
                                Errno::EISCONN,
                                "the socket is listening",
                            ))),
                            None,
                        ),
                    );
                }
            };

            let connecting_stream = match init_stream.connect(
                remote_endpoint,
                &raw_option,
                StreamObserver::new(self.pollee.clone()),
            ) {
                Ok(connecting_stream) => connecting_stream,
                Err((mut err, init_stream)) => {
                    // If the socket is nonblocking, we should return EINPROGRESS instead.
                    if is_nonblocking {
                        options.socket.set_sock_errors(Some(err));
                        err = Error::new(Errno::EINPROGRESS);
                    }

                    return (State::Init(init_stream), (Some(Err(err)), None));
                }
            };

            let result_or_block = if is_nonblocking {
                Some(Err(Error::with_message(
                    Errno::EINPROGRESS,
                    "the socket is connecting",
                )))
            } else {
                None
            };
            let iface_to_poll = connecting_stream.iface().clone();

            (
                State::Connecting(connecting_stream),
                (result_or_block, Some(iface_to_poll)),
            )
        });

        drop(state);
        self.pollee.invalidate();
        if let Some(iface) = iface_to_poll {
            iface.poll();
        }

        result_or_block
    }

    fn check_connect(&self) -> Result<()> {
        let (mut options, mut state) = self.update_connecting();

        match state.as_mut() {
            State::Connecting(_) => {
                return_errno_with_message!(Errno::EAGAIN, "the connection is pending")
            }
            State::Connected(connected_stream) => connected_stream.check_new(),
            State::Init(_) | State::Listen(_) => {
                let sock_errors = options.socket.sock_errors();
                options.socket.set_sock_errors(None);
                sock_errors.map(Err).unwrap_or(Ok(()))
            }
        }
    }

    fn try_accept(&self) -> Result<(Arc<dyn FileLike>, SocketAddr)> {
        let state = self.read_updated_state();

        let State::Listen(listen_stream) = state.as_ref() else {
            return_errno_with_message!(Errno::EINVAL, "the socket is not listening");
        };

        let accepted = listen_stream.try_accept().map(|connected_stream| {
            let remote_endpoint = connected_stream.remote_endpoint();
            let accepted_socket = Self::new_accepted(connected_stream);
            (accepted_socket as _, remote_endpoint.into())
        });
        let iface_to_poll = listen_stream.iface().clone();

        drop(state);
        self.pollee.invalidate();
        iface_to_poll.poll();

        accepted
    }

    fn try_recv(
        &self,
        writer: &mut dyn MultiWrite,
        flags: SendRecvFlags,
    ) -> Result<(usize, SocketAddr)> {
        let state = self.read_updated_state();

        let connected_stream = match state.as_ref() {
            State::Connected(connected_stream) => connected_stream,
            State::Init(_) | State::Listen(_) => {
                return_errno_with_message!(Errno::ENOTCONN, "the socket is not connected")
            }
            State::Connecting(_) => {
                return_errno_with_message!(Errno::EAGAIN, "the socket is connecting")
            }
        };

        let (recv_bytes, need_poll) = connected_stream.try_recv(writer, flags)?;
        let iface_to_poll = need_poll.then(|| connected_stream.iface().clone());
        let remote_endpoint = connected_stream.remote_endpoint();

        drop(state);
        self.pollee.invalidate();
        if let Some(iface) = iface_to_poll {
            iface.poll();
        }

        Ok((recv_bytes, remote_endpoint.into()))
    }

    fn try_send(&self, reader: &mut dyn MultiRead, flags: SendRecvFlags) -> Result<usize> {
        let state = self.read_updated_state();

        let connected_stream = match state.as_ref() {
            State::Connected(connected_stream) => connected_stream,
            State::Init(_) | State::Listen(_) => {
                // TODO: Trigger `SIGPIPE` if `MSG_NOSIGNAL` is not specified
                return_errno_with_message!(Errno::EPIPE, "the socket is not connected");
            }
            State::Connecting(_) => {
                // FIXME: Linux indeed allows data to be buffered at this point. Can we do
                // something similar?
                return_errno_with_message!(Errno::EAGAIN, "the socket is connecting")
            }
        };

        let (sent_bytes, need_poll) = connected_stream.try_send(reader, flags)?;
        let iface_to_poll = need_poll.then(|| connected_stream.iface().clone());

        drop(state);
        self.pollee.invalidate();
        if let Some(iface) = iface_to_poll {
            iface.poll();
        }

        Ok(sent_bytes)
    }

    fn check_io_events(&self) -> IoEvents {
        let state = self.read_updated_state();

        match state.as_ref() {
            State::Init(init_stream) => init_stream.check_io_events(),
            State::Connecting(connecting_stream) => connecting_stream.check_io_events(),
            State::Listen(listen_stream) => listen_stream.check_io_events(),
            State::Connected(connected_stream) => connected_stream.check_io_events(),
        }
    }
}

impl Pollable for StreamSocket {
    fn poll(&self, mask: IoEvents, poller: Option<&mut PollHandle>) -> IoEvents {
        self.pollee
            .poll_with(mask, poller, || self.check_io_events())
    }
}

impl SocketPrivate for StreamSocket {
    fn is_nonblocking(&self) -> bool {
        self.is_nonblocking.load(Ordering::Relaxed)
    }

    fn set_nonblocking(&self, nonblocking: bool) {
        self.is_nonblocking.store(nonblocking, Ordering::Relaxed);
    }
}

impl Socket for StreamSocket {
    fn bind(&self, socket_addr: SocketAddr) -> Result<()> {
        let endpoint = socket_addr.try_into()?;

        let can_reuse = self.options.read().socket.reuse_addr();
        let mut state = self.write_updated_state();

        state.borrow_result(|owned_state| {
            let State::Init(init_stream) = owned_state else {
                return (
                    owned_state,
                    Err(Error::with_message(
                        Errno::EINVAL,
                        "the socket is already bound to an address",
                    )),
                );
            };

            let bound_port = match init_stream.bind(&endpoint, can_reuse) {
                Ok(bound_port) => bound_port,
                Err((err, init_stream)) => {
                    return (State::Init(init_stream), Err(err));
                }
            };

            (State::Init(InitStream::new_bound(bound_port)), Ok(()))
        })
    }

    fn connect(&self, socket_addr: SocketAddr) -> Result<()> {
        let remote_endpoint = socket_addr.try_into()?;

        if let Some(result) = self.start_connect(&remote_endpoint) {
            return result;
        }

        self.wait_events(IoEvents::OUT, None, || self.check_connect())
    }

    fn listen(&self, backlog: usize) -> Result<()> {
        let (options, mut state) = self.update_connecting();

        let raw_option = options.raw();

        state.borrow_result(|owned_state| {
            let init_stream = match owned_state {
                State::Init(init_stream) => init_stream,
                State::Listen(listen_stream) => {
                    return (State::Listen(listen_stream), Ok(()));
                }
                State::Connecting(_) | State::Connected(_) => {
                    return (
                        owned_state,
                        Err(Error::with_message(
                            Errno::EINVAL,
                            "the socket is already connected",
                        )),
                    );
                }
            };

            let listen_stream = match init_stream.listen(
                backlog,
                &raw_option,
                StreamObserver::new(self.pollee.clone()),
            ) {
                Ok(listen_stream) => listen_stream,
                Err((err, init_stream)) => {
                    return (State::Init(init_stream), Err(err));
                }
            };

            self.pollee.invalidate();
            (State::Listen(listen_stream), Ok(()))
        })
    }

    fn accept(&self) -> Result<(Arc<dyn FileLike>, SocketAddr)> {
        self.block_on(IoEvents::IN, || self.try_accept())
    }

    fn shutdown(&self, cmd: SockShutdownCmd) -> Result<()> {
        let state = self.read_updated_state();

        let (result, iface_to_poll) = match state.as_ref() {
            State::Connected(connected_stream) => (
                connected_stream.shutdown(cmd, &self.pollee),
                connected_stream.iface().clone(),
            ),
            // TODO: shutdown listening stream
            _ => return_errno_with_message!(Errno::EINVAL, "cannot shutdown"),
        };

        drop(state);
        // No need to call `Pollee::invalidate` because `ConnectedStream::shutdown` will call
        // `Pollee::notify`.
        iface_to_poll.poll();

        result
    }

    fn addr(&self) -> Result<SocketAddr> {
        let state = self.read_updated_state();
        let local_endpoint = match state.as_ref() {
            State::Init(init_stream) => init_stream
                .local_endpoint()
                .unwrap_or(UNSPECIFIED_LOCAL_ENDPOINT),
            State::Connecting(connecting_stream) => connecting_stream.local_endpoint(),
            State::Listen(listen_stream) => listen_stream.local_endpoint(),
            State::Connected(connected_stream) => connected_stream.local_endpoint(),
        };
        Ok(local_endpoint.into())
    }

    fn peer_addr(&self) -> Result<SocketAddr> {
        let state = self.read_updated_state();
        let remote_endpoint = match state.as_ref() {
            State::Init(_) | State::Listen(_) => {
                return_errno_with_message!(Errno::ENOTCONN, "the socket is not connected")
            }
            State::Connecting(connecting_stream) => connecting_stream.remote_endpoint(),
            State::Connected(connected_stream) => connected_stream.remote_endpoint(),
        };
        Ok(remote_endpoint.into())
    }

    fn sendmsg(
        &self,
        reader: &mut dyn MultiRead,
        message_header: MessageHeader,
        flags: SendRecvFlags,
    ) -> Result<usize> {
        // TODO: Deal with flags
        if !flags.is_all_supported() {
            warn!("unsupported flags: {:?}", flags);
        }

        let MessageHeader {
            control_message, ..
        } = message_header;

        // According to the Linux man pages, `EISCONN` _may_ be returned when the destination
        // address is specified for a connection-mode socket. In practice, the destination address
        // is simply ignored. We follow the same behavior as the Linux implementation to ignore it.

        if control_message.is_some() {
            // TODO: Support sending control message
            warn!("sending control message is not supported");
        }

        self.block_on(IoEvents::OUT, || self.try_send(reader, flags))
    }

    fn recvmsg(
        &self,
        writer: &mut dyn MultiWrite,
        flags: SendRecvFlags,
    ) -> Result<(usize, MessageHeader)> {
        // TODO: Deal with flags
        if !flags.is_all_supported() {
            warn!("unsupported flags: {:?}", flags);
        }

        let (received_bytes, _) = self.block_on(IoEvents::IN, || self.try_recv(writer, flags))?;

        // TODO: Receive control message

        // According to <https://elixir.bootlin.com/linux/v6.0.9/source/net/ipv4/tcp.c#L2645>,
        // peer address is ignored for connected socket.
        let message_header = MessageHeader::new(None, None);

        Ok((received_bytes, message_header))
    }

    fn get_option(&self, option: &mut dyn SocketOption) -> Result<()> {
        match_sock_option_mut!(option, {
            socket_errors: SocketError => {
                let mut options = self.update_connecting().0;
                options.socket.get_and_clear_sock_errors(socket_errors);
                return Ok(());
            },
            _ => ()
        });

        let options = self.options.read();

        match options.socket.get_option(option) {
            Err(err) if err.error() == Errno::ENOPROTOOPT => (),
            res => return res,
        }

        // FIXME: Here we only return the previously set values, without actually
        // asking the underlying sockets for the real, effective values.
        match_sock_option_mut!(option, {
            tcp_no_delay: NoDelay => {
                let no_delay = options.tcp.no_delay();
                tcp_no_delay.set(no_delay);
            },
            tcp_maxseg: MaxSegment => {
                const DEFAULT_MAX_SEGMEMT: u32 = 536;
                // For an unconnected socket,
                // older Linux versions (e.g., v6.0) return
                // the default MSS value defined above.
                // However, newer Linux versions (e.g., v6.11)
                // return the user-set MSS value if it is set.
                // Here, we adopt the behavior of the latest Linux versions.
                let maxseg = options.tcp.maxseg();
                if maxseg == 0 {
                    tcp_maxseg.set(DEFAULT_MAX_SEGMEMT);
                } else {
                    tcp_maxseg.set(maxseg);
                }
            },
            tcp_keep_idle: KeepIdle => {
                let keep_idle = options.tcp.keep_idle();
                tcp_keep_idle.set(keep_idle);
            },
            tcp_syn_cnt: SynCnt => {
                let syn_cnt = options.tcp.syn_cnt();
                tcp_syn_cnt.set(syn_cnt);
            },
            tcp_defer_accept: DeferAccept => {
                let defer_accept = options.tcp.defer_accept();
                let seconds = defer_accept.to_secs();
                tcp_defer_accept.set(seconds);
            },
            tcp_window_clamp: WindowClamp => {
                let window_clamp = options.tcp.window_clamp();
                tcp_window_clamp.set(window_clamp);
            },
            tcp_congestion: Congestion => {
                let congestion = options.tcp.congestion();
                tcp_congestion.set(congestion);
            },
            tcp_user_timeout: UserTimeout => {
                let user_timeout = options.tcp.user_timeout();
                tcp_user_timeout.set(user_timeout);
            },
            tcp_inq: Inq => {
                let inq = options.tcp.receive_inq();
                tcp_inq.set(inq);
            },
            _ => return_errno_with_message!(Errno::ENOPROTOOPT, "the socket option to get is unknown")
        });

        Ok(())
    }

    fn set_option(&self, option: &dyn SocketOption) -> Result<()> {
        let (mut options, mut state) = self.update_connecting();

        let need_iface_poll = match options.socket.set_option(option, state.as_mut()) {
            Err(err) if err.error() == Errno::ENOPROTOOPT => {
                do_tcp_setsockopt(option, &mut options, state.as_mut())?
            }
            Err(err) => return Err(err),
            Ok(need_iface_poll) => need_iface_poll,
        };

        let iface_to_poll = need_iface_poll.then(|| state.iface().cloned()).flatten();

        drop(state);
        drop(options);

        if let Some(iface) = iface_to_poll {
            iface.poll();
        }

        Ok(())
    }
}

fn do_tcp_setsockopt(
    option: &dyn SocketOption,
    options: &mut OptionSet,
    state: &mut State,
) -> Result<NeedIfacePoll> {
    match_sock_option_ref!(option, {
        tcp_no_delay: NoDelay => {
            let no_delay = tcp_no_delay.get().unwrap();
            options.tcp.set_no_delay(*no_delay);
            state.set_raw_option(|raw_socket: &dyn RawTcpSetOption| raw_socket.set_nagle_enabled(!no_delay));
        },
        tcp_maxseg: MaxSegment => {
            const MIN_MAXSEG: u32 = 536;
            const MAX_MAXSEG: u32 = 65535;

            let maxseg = tcp_maxseg.get().unwrap();
            if *maxseg < MIN_MAXSEG || *maxseg > MAX_MAXSEG {
                return_errno_with_message!(Errno::EINVAL, "the maximum segment size is out of bounds");
            }
            options.tcp.set_maxseg(*maxseg);
        },
        tcp_keep_idle: KeepIdle => {
            const MIN_KEEP_IDLE: u32 = 1;
            const MAX_KEEP_IDLE: u32 = 32767;

            let keepidle = tcp_keep_idle.get().unwrap();
            if *keepidle < MIN_KEEP_IDLE || *keepidle > MAX_KEEP_IDLE {
                return_errno_with_message!(Errno::EINVAL, "the keep idle time is out of bounds");
            }
            options.tcp.set_keep_idle(*keepidle);

            // TODO: Track when the socket becomes idle to actually support keep idle.
        },
        tcp_syn_cnt: SynCnt => {
            const MAX_TCP_SYN_CNT: u8 = 127;

            let syncnt = tcp_syn_cnt.get().unwrap();
            if *syncnt < 1 || *syncnt > MAX_TCP_SYN_CNT {
                return_errno_with_message!(Errno::EINVAL, "the SYN count is out of bounds");
            }
            options.tcp.set_syn_cnt(*syncnt);
        },
        tcp_defer_accept: DeferAccept => {
            let mut seconds = *(tcp_defer_accept.get().unwrap());
            if (seconds as i32) < 0 {
                seconds = 0;
            }
            let retrans = Retrans::from_secs(seconds);
            options.tcp.set_defer_accept(retrans);
        },
        tcp_window_clamp: WindowClamp => {
            let window_clamp = tcp_window_clamp.get().unwrap();
            let half_recv_buf = options.socket.recv_buf() / 2;
            if *window_clamp <= half_recv_buf {
                options.tcp.set_window_clamp(half_recv_buf);
            } else {
                options.tcp.set_window_clamp(*window_clamp);
            }
        },
        tcp_congestion: Congestion => {
            let congestion = tcp_congestion.get().unwrap();
            options.tcp.set_congestion(*congestion);
        },
        tcp_user_timeout: UserTimeout => {
            let user_timeout = tcp_user_timeout.get().unwrap();
            if (*user_timeout as i32) < 0 {
                return_errno_with_message!(Errno::EINVAL, "the user timeout cannot be negative");
            }
            options.tcp.set_user_timeout(*user_timeout);
        },
        tcp_inq: Inq => {
            let inq = tcp_inq.get().unwrap();
            options.tcp.set_receive_inq(*inq);
        },
        _ => return_errno_with_message!(Errno::ENOPROTOOPT, "the socket option to be set is unknown")
    });

    Ok(NeedIfacePoll::FALSE)
}

impl State {
    /// Calls `f` to set raw socket option.
    ///
    /// For listening sockets, socket options are inherited by new connections. However, they are
    /// not updated for connections in the backlog queue.
    fn set_raw_option<R>(&self, set_option: impl FnOnce(&dyn RawTcpSetOption) -> R) -> Option<R> {
        match self {
            State::Init(_) => None,
            State::Connecting(connecting_stream) => {
                Some(connecting_stream.set_raw_option(set_option))
            }
            State::Connected(connected_stream) => Some(connected_stream.set_raw_option(set_option)),
            State::Listen(listen_stream) => Some(listen_stream.set_raw_option(set_option)),
        }
    }

    fn iface(&self) -> Option<&Arc<Iface>> {
        match self {
            State::Init(_) => None,
            State::Connecting(ref connecting_stream) => Some(connecting_stream.iface()),
            State::Connected(ref connected_stream) => Some(connected_stream.iface()),
            State::Listen(ref listen_stream) => Some(listen_stream.iface()),
        }
    }
}

impl SetSocketLevelOption for State {
    fn set_keep_alive(&self, keep_alive: bool) -> NeedIfacePoll {
        let interval = if keep_alive {
            Some(KEEPALIVE_INTERVAL)
        } else {
            None
        };

        let set_keepalive = |raw_socket: &dyn RawTcpSetOption| raw_socket.set_keep_alive(interval);

        self.set_raw_option(set_keepalive)
            .unwrap_or(NeedIfacePoll::FALSE)
    }
}

impl Drop for StreamSocket {
    fn drop(&mut self) {
        let state = self.state.get_mut().take();

        let iface_to_poll = state.iface().cloned();

        // Dropping the state will drop the sockets. This will trigger the socket close process (if
        // needed) and require immediate iface polling afterwards.
        drop(state);

        if let Some(iface) = iface_to_poll {
            iface.poll();
        }
    }
}
