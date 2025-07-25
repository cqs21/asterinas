// SPDX-License-Identifier: MPL-2.0

use bitflags::bitflags;

use super::{
    c_types::sigaction_t,
    constants::*,
    sig_mask::{SigMask, SigSet},
    sig_num::SigNum,
};
use crate::prelude::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum SigAction {
    #[default]
    Dfl, // Default action
    Ign, // Ignore this signal
    User {
        // User-given handler
        handler_addr: usize,
        flags: SigActionFlags,
        restorer_addr: usize,
        mask: SigMask,
    },
}

impl From<sigaction_t> for SigAction {
    fn from(input: sigaction_t) -> Self {
        match input.handler_ptr {
            SIG_DFL => SigAction::Dfl,
            SIG_IGN => SigAction::Ign,
            _ => {
                let flags = SigActionFlags::from_bits_truncate(input.flags);
                let mask = {
                    let mut sigset = SigSet::from(input.mask);
                    // SIGSTOP and SIGKILL cannot be masked
                    sigset -= SIGSTOP;
                    sigset -= SIGKILL;
                    sigset
                };
                SigAction::User {
                    handler_addr: input.handler_ptr,
                    flags,
                    restorer_addr: input.restorer_ptr,
                    mask,
                }
            }
        }
    }
}

impl SigAction {
    pub fn as_c_type(&self) -> sigaction_t {
        match self {
            SigAction::Dfl => sigaction_t {
                handler_ptr: SIG_DFL,
                flags: 0,
                restorer_ptr: 0,
                mask: 0,
            },
            SigAction::Ign => sigaction_t {
                handler_ptr: SIG_IGN,
                flags: 0,
                restorer_ptr: 0,
                mask: 0,
            },
            SigAction::User {
                handler_addr,
                flags,
                restorer_addr,
                mask,
            } => sigaction_t {
                handler_ptr: *handler_addr,
                flags: flags.as_u32(),
                restorer_ptr: *restorer_addr,
                mask: (*mask).into(),
            },
        }
    }

    /// Returns whether signals will be ignored.
    ///
    /// Signals will be ignored because either
    ///  * the signal action is explicitly set to ignore the signals, or
    ///  * the signal action is default and the default action is to ignore the signals.
    pub fn will_ignore(&self, signum: SigNum) -> bool {
        match self {
            SigAction::Dfl => {
                let default_action = SigDefaultAction::from_signum(signum);
                matches!(default_action, SigDefaultAction::Ign)
            }
            SigAction::Ign => true,
            SigAction::User { .. } => false,
        }
    }
}

bitflags! {
    pub struct SigActionFlags: u32 {
        const SA_NOCLDSTOP  = 1;
        const SA_NOCLDWAIT  = 2;
        const SA_SIGINFO    = 4;
        const SA_ONSTACK    = 0x08000000;
        const SA_RESTART    = 0x10000000;
        const SA_NODEFER    = 0x40000000;
        const SA_RESETHAND  = 0x80000000;
        const SA_RESTORER   = 0x04000000;
    }
}

impl TryFrom<u32> for SigActionFlags {
    type Error = Error;

    fn try_from(bits: u32) -> Result<Self> {
        let flags = SigActionFlags::from_bits(bits)
            .ok_or_else(|| Error::with_message(Errno::EINVAL, "invalid sig action flag"))?;
        Ok(flags)
    }
}

impl SigActionFlags {
    pub fn as_u32(&self) -> u32 {
        self.bits()
    }

    pub fn contains_unsupported_flag(&self) -> bool {
        self.intersects(SigActionFlags::SA_NOCLDSTOP | SigActionFlags::SA_NOCLDWAIT)
    }
}

/// The default action to signals
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SigDefaultAction {
    Term, // Default action is to terminate the process.
    Ign,  // Default action is to ignore the signal.
    Core, // Default action is to terminate the process and dump core (see core(5)).
    Stop, // Default action is to stop the process.
    Cont, // Default action is to continue the process if it is currently stopped.
}

impl SigDefaultAction {
    pub fn from_signum(num: SigNum) -> SigDefaultAction {
        match num {
            SIGABRT | // = SIGIOT
            SIGBUS  |
            SIGFPE  |
            SIGILL  |
            SIGQUIT |
            SIGSEGV |
            SIGSYS  | // = SIGUNUSED
            SIGTRAP |
            SIGXCPU |
            SIGXFSZ
                => SigDefaultAction::Core,
            SIGCHLD |
            SIGURG  |
            SIGWINCH
                => SigDefaultAction::Ign,
            SIGCONT
                => SigDefaultAction::Cont,
            SIGSTOP |
            SIGTSTP |
            SIGTTIN |
            SIGTTOU
                => SigDefaultAction::Stop,
            _
                => SigDefaultAction::Term,
        }
    }
}
