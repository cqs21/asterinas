// SPDX-License-Identifier: MPL-2.0

use bitflags::bitflags;

use super::{
    c_types::sigaction_t,
    constants::*,
    sig_mask::{SigMask, SigSet},
    sig_num::SigNum,
};
use crate::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SigAction {
    Dfl {
        // Default action
        flags: SigActionFlags,
        restorer_addr: usize,
        mask: SigMask,
    },
    Ign {
        // Ignore this signal
        flags: SigActionFlags,
        restorer_addr: usize,
        mask: SigMask,
    },
    User {
        // User-given handler
        handler_addr: usize,
        flags: SigActionFlags,
        restorer_addr: usize,
        mask: SigMask,
    },
}

impl Default for SigAction {
    fn default() -> Self {
        Self::Dfl {
            flags: SigActionFlags::empty(),
            restorer_addr: 0,
            mask: SigMask::new_empty(),
        }
    }
}

impl From<sigaction_t> for SigAction {
    fn from(input: sigaction_t) -> Self {
        let flags = SigActionFlags::from_bits_truncate(input.flags);
        let mask = SigSet::from(input.mask);
        let restorer_addr = input.restorer_ptr;

        match input.handler_ptr {
            SIG_DFL => SigAction::Dfl {
                flags,
                restorer_addr,
                mask,
            },
            SIG_IGN => SigAction::Ign {
                flags,
                restorer_addr,
                mask,
            },
            _ => SigAction::User {
                handler_addr: input.handler_ptr,
                flags,
                restorer_addr,
                mask,
            },
        }
    }
}

impl SigAction {
    pub fn as_c_type(&self) -> sigaction_t {
        match self {
            SigAction::Dfl {
                flags,
                restorer_addr,
                mask,
            } => sigaction_t {
                handler_ptr: SIG_DFL,
                flags: flags.as_u32(),
                restorer_ptr: *restorer_addr,
                mask: (*mask).into(),
                ..Default::default()
            },
            SigAction::Ign {
                flags,
                restorer_addr,
                mask,
            } => sigaction_t {
                handler_ptr: SIG_IGN,
                flags: flags.as_u32(),
                restorer_ptr: *restorer_addr,
                mask: (*mask).into(),
                ..Default::default()
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
                ..Default::default()
            },
        }
    }

    /// Resets a user-installed handler to the default disposition while preserving metadata.
    ///
    /// Linux keeps the original `sa_flags`, `sa_mask`, and `sa_restorer` visible through
    /// `sigaction(..., NULL, &oldact)` even after `SA_RESETHAND` has reset the handler.
    pub fn reset_user_handler(self) -> Self {
        match self {
            SigAction::User {
                flags,
                restorer_addr,
                mask,
                ..
            } => SigAction::Dfl {
                flags,
                restorer_addr,
                mask,
            },
            action => action,
        }
    }

    /// Returns whether signals will be ignored.
    ///
    /// Signals will be ignored because either
    ///  * the signal action is explicitly set to ignore the signals, or
    ///  * the signal action is default and the default action is to ignore the signals.
    pub fn will_ignore(&self, signum: SigNum) -> bool {
        match self {
            SigAction::Dfl { .. } => {
                let default_action = SigDefaultAction::from_signum(signum);
                matches!(default_action, SigDefaultAction::Ign)
            }
            SigAction::Ign { .. } => true,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
