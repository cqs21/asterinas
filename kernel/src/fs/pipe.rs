// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::{AtomicU32, Ordering};

use super::{
    file_handle::FileLike,
    utils::{AccessMode, Endpoint, EndpointState, InodeMode, InodeType, Metadata, StatusFlags},
};
use crate::{
    events::IoEvents,
    prelude::*,
    process::{
        signal::{PollHandle, Pollable},
        Gid, Uid,
    },
    time::clocks::RealTimeCoarseClock,
    util::ring_buffer::{RbConsumer, RbProducer, RingBuffer},
};

const DEFAULT_PIPE_BUF_SIZE: usize = 65536;

/// Maximum number of bytes guaranteed to be written to a pipe atomically.
///
/// If the number of bytes to be written is less than the threshold, the write must be atomic.
/// A non-blocking atomic write may fail with `EAGAIN`, even if there is room for a partial write.
/// In other words, a partial write is not allowed for an atomic write.
///
/// For more details, see the description of `PIPE_BUF` in
/// <https://man7.org/linux/man-pages/man7/pipe.7.html>.
#[cfg(not(ktest))]
const PIPE_BUF: usize = 4096;
#[cfg(ktest)]
const PIPE_BUF: usize = 2;

pub fn new_pair() -> Result<(Arc<PipeReader>, Arc<PipeWriter>)> {
    new_pair_with_capacity(DEFAULT_PIPE_BUF_SIZE)
}

pub fn new_pair_with_capacity(capacity: usize) -> Result<(Arc<PipeReader>, Arc<PipeWriter>)> {
    let (producer, consumer) = RingBuffer::new(capacity).split();
    let (producer_state, consumer_state) =
        Endpoint::new_pair(EndpointState::default(), EndpointState::default());

    Ok((
        PipeReader::new(consumer, consumer_state, StatusFlags::empty())?,
        PipeWriter::new(producer, producer_state, StatusFlags::empty())?,
    ))
}

pub struct PipeReader {
    consumer: Mutex<RbConsumer<u8>>,
    state: Endpoint<EndpointState>,
    status_flags: AtomicU32,
}

impl PipeReader {
    fn new(
        consumer: RbConsumer<u8>,
        state: Endpoint<EndpointState>,
        status_flags: StatusFlags,
    ) -> Result<Arc<Self>> {
        check_status_flags(status_flags)?;

        Ok(Arc::new(Self {
            consumer: Mutex::new(consumer),
            state,
            status_flags: AtomicU32::new(status_flags.bits()),
        }))
    }

    fn try_read(&self, writer: &mut VmWriter) -> Result<usize> {
        let read = || {
            let mut consumer = self.consumer.lock();
            consumer.read_fallible(writer)
        };

        self.state.read_with(read)
    }

    fn check_io_events(&self) -> IoEvents {
        let mut events = IoEvents::empty();
        if self.state.is_peer_shutdown() {
            events |= IoEvents::HUP;
        }
        if !self.consumer.lock().is_empty() {
            events |= IoEvents::IN;
        }
        events
    }
}

impl Pollable for PipeReader {
    fn poll(&self, mask: IoEvents, poller: Option<&mut PollHandle>) -> IoEvents {
        self.state
            .poll_with(mask, poller, || self.check_io_events())
    }
}

impl FileLike for PipeReader {
    fn read(&self, writer: &mut VmWriter) -> Result<usize> {
        if !writer.has_avail() {
            // Even the peer endpoint (`PipeWriter`) has been closed, reading an empty buffer is
            // still fine.
            return Ok(0);
        }

        if self.status_flags().contains(StatusFlags::O_NONBLOCK) {
            self.try_read(writer)
        } else {
            self.wait_events(IoEvents::IN, None, || self.try_read(writer))
        }
    }

    fn status_flags(&self) -> StatusFlags {
        StatusFlags::from_bits_truncate(self.status_flags.load(Ordering::Relaxed))
    }

    fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        check_status_flags(new_flags)?;

        self.status_flags.store(new_flags.bits(), Ordering::Relaxed);
        Ok(())
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_RDONLY
    }

    fn metadata(&self) -> Metadata {
        // This is a dummy implementation.
        // TODO: Add "PipeFS" and link `PipeReader` to it.
        let now = RealTimeCoarseClock::get().read_time();
        Metadata {
            dev: 0,
            ino: 0,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            type_: InodeType::NamedPipe,
            mode: InodeMode::from_bits_truncate(0o400),
            nlinks: 1,
            uid: Uid::new_root(),
            gid: Gid::new_root(),
            rdev: 0,
        }
    }
}

impl Drop for PipeReader {
    fn drop(&mut self) {
        self.state.peer_shutdown();
    }
}

pub struct PipeWriter {
    producer: Mutex<RbProducer<u8>>,
    state: Endpoint<EndpointState>,
    status_flags: AtomicU32,
}

impl PipeWriter {
    fn new(
        producer: RbProducer<u8>,
        state: Endpoint<EndpointState>,
        status_flags: StatusFlags,
    ) -> Result<Arc<Self>> {
        check_status_flags(status_flags)?;

        Ok(Arc::new(Self {
            producer: Mutex::new(producer),
            state,
            status_flags: AtomicU32::new(status_flags.bits()),
        }))
    }

    fn try_write(&self, reader: &mut VmReader) -> Result<usize> {
        let write = || {
            let mut producer = self.producer.lock();
            if reader.remain() <= PIPE_BUF && producer.free_len() < reader.remain() {
                // No sufficient space for an atomic write
                return Ok(0);
            }
            producer.write_fallible(reader)
        };

        self.state.write_with(write)
    }

    fn check_io_events(&self) -> IoEvents {
        if self.state.is_shutdown() {
            IoEvents::ERR | IoEvents::OUT
        } else if self.producer.lock().free_len() >= PIPE_BUF {
            IoEvents::OUT
        } else {
            IoEvents::empty()
        }
    }
}

impl Pollable for PipeWriter {
    fn poll(&self, mask: IoEvents, poller: Option<&mut PollHandle>) -> IoEvents {
        self.state
            .poll_with(mask, poller, || self.check_io_events())
    }
}

impl FileLike for PipeWriter {
    fn write(&self, reader: &mut VmReader) -> Result<usize> {
        if !reader.has_remain() {
            // Even the peer endpoint (`PipeReader`) has been closed, writing an empty buffer is
            // still fine.
            return Ok(0);
        }

        if self.status_flags().contains(StatusFlags::O_NONBLOCK) {
            self.try_write(reader)
        } else {
            self.wait_events(IoEvents::OUT, None, || self.try_write(reader))
        }
    }

    fn status_flags(&self) -> StatusFlags {
        StatusFlags::from_bits_truncate(self.status_flags.load(Ordering::Relaxed))
    }

    fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        check_status_flags(new_flags)?;

        self.status_flags.store(new_flags.bits(), Ordering::Relaxed);
        Ok(())
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_WRONLY
    }

    fn metadata(&self) -> Metadata {
        // This is a dummy implementation.
        // TODO: Add "PipeFS" and link `PipeWriter` to it.
        let now = RealTimeCoarseClock::get().read_time();
        Metadata {
            dev: 0,
            ino: 0,
            size: 0,
            blk_size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            type_: InodeType::NamedPipe,
            mode: InodeMode::from_bits_truncate(0o200),
            nlinks: 1,
            uid: Uid::new_root(),
            gid: Gid::new_root(),
            rdev: 0,
        }
    }
}

fn check_status_flags(status_flags: StatusFlags) -> Result<()> {
    if status_flags.contains(StatusFlags::O_DIRECT) {
        // "O_DIRECT .. Older kernels that do not support this flag will indicate this via an
        // EINVAL error."
        //
        // See <https://man7.org/linux/man-pages/man2/pipe.2.html>.
        return_errno_with_message!(Errno::EINVAL, "the `O_DIRECT` flag is not supported");
    }

    // TODO: Setting most of the other flags will succeed on Linux, but their effects need to be
    // validated.

    Ok(())
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        self.state.shutdown();
    }
}

#[cfg(ktest)]
mod test {
    use alloc::sync::Arc;
    use core::sync::atomic::{self, AtomicBool};

    use ostd::prelude::*;

    use super::*;
    use crate::thread::{kernel_thread::ThreadOptions, Thread};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Ordering {
        WriteThenRead,
        ReadThenWrite,
    }

    fn test_blocking<W, R>(write: W, read: R, ordering: Ordering)
    where
        W: FnOnce(Arc<PipeWriter>) + Send + 'static,
        R: FnOnce(Arc<PipeReader>) + Send + 'static,
    {
        let (reader, writer) = new_pair_with_capacity(2).unwrap();

        let signal_writer = Arc::new(AtomicBool::new(false));
        let signal_reader = signal_writer.clone();

        let writer = ThreadOptions::new(move || {
            if ordering == Ordering::ReadThenWrite {
                while !signal_writer.load(atomic::Ordering::Relaxed) {
                    Thread::yield_now();
                }
            } else {
                signal_writer.store(true, atomic::Ordering::Relaxed);
            }

            write(writer);
        })
        .spawn();

        let reader = ThreadOptions::new(move || {
            if ordering == Ordering::WriteThenRead {
                while !signal_reader.load(atomic::Ordering::Relaxed) {
                    Thread::yield_now();
                }
            } else {
                signal_reader.store(true, atomic::Ordering::Relaxed);
            }

            read(reader);
        })
        .spawn();

        writer.join();
        reader.join();
    }

    #[ktest]
    fn test_read_empty() {
        test_blocking(
            |writer| {
                assert_eq!(writer.write(&mut reader_from(&[1])).unwrap(), 1);
            },
            |reader| {
                let mut buf = [0; 1];
                assert_eq!(reader.read(&mut writer_from(&mut buf)).unwrap(), 1);
                assert_eq!(&buf, &[1]);
            },
            Ordering::ReadThenWrite,
        );
    }

    #[ktest]
    fn test_write_full() {
        test_blocking(
            |writer| {
                assert_eq!(writer.write(&mut reader_from(&[1, 2, 3])).unwrap(), 2);
                assert_eq!(writer.write(&mut reader_from(&[2])).unwrap(), 1);
            },
            |reader| {
                let mut buf = [0; 3];
                assert_eq!(reader.read(&mut writer_from(&mut buf)).unwrap(), 2);
                assert_eq!(&buf[..2], &[1, 2]);
                assert_eq!(reader.read(&mut writer_from(&mut buf)).unwrap(), 1);
                assert_eq!(&buf[..1], &[2]);
            },
            Ordering::WriteThenRead,
        );
    }

    #[ktest]
    fn test_read_closed() {
        test_blocking(
            drop,
            |reader| {
                let mut buf = [0; 1];
                assert_eq!(reader.read(&mut writer_from(&mut buf)).unwrap(), 0);
            },
            Ordering::ReadThenWrite,
        );
    }

    #[ktest]
    fn test_write_closed() {
        test_blocking(
            |writer| {
                assert_eq!(writer.write(&mut reader_from(&[1, 2, 3])).unwrap(), 2);
                assert_eq!(
                    writer.write(&mut reader_from(&[2])).unwrap_err().error(),
                    Errno::EPIPE
                );
            },
            drop,
            Ordering::WriteThenRead,
        );
    }

    #[ktest]
    fn test_write_atomicity() {
        test_blocking(
            |writer| {
                assert_eq!(writer.write(&mut reader_from(&[1])).unwrap(), 1);
                assert_eq!(writer.write(&mut reader_from(&[1, 2])).unwrap(), 2);
            },
            |reader| {
                let mut buf = [0; 3];
                assert_eq!(reader.read(&mut writer_from(&mut buf)).unwrap(), 1);
                assert_eq!(&buf[..1], &[1]);
                assert_eq!(reader.read(&mut writer_from(&mut buf)).unwrap(), 2);
                assert_eq!(&buf[..2], &[1, 2]);
            },
            Ordering::WriteThenRead,
        );
    }

    fn reader_from(buf: &[u8]) -> VmReader {
        VmReader::from(buf).to_fallible()
    }

    fn writer_from(buf: &mut [u8]) -> VmWriter {
        VmWriter::from(buf).to_fallible()
    }
}
