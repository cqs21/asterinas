// SPDX-License-Identifier: MPL-2.0

use core::{sync::atomic::Ordering, time::Duration};

use ostd::sync::Waiter;

use super::SyscallReturn;
use crate::{
    prelude::*,
    process::signal::{
        constants::{SIGKILL, SIGSTOP},
        sig_mask::{SigMask, SigSet},
        signals::Signal,
    },
    time::{clocks::MonotonicClock, timer::Timeout, timespec_t, wait::ManagedTimeout},
};

pub fn sys_rt_sigtimedwait(
    sigset: Vaddr,
    siginfo: Vaddr,
    timeout: Vaddr,
    sigset_size: u64,
    ctx: &Context,
) -> Result<SyscallReturn> {
    if sigset_size != 8 {
        return_errno_with_message!(Errno::EINVAL, "sigset size is not equal to 8");
    }

    let sigset = ctx.user_space().read_val::<SigSet>(sigset)?;
    let mask = !(sigset - SIGKILL - SIGSTOP);

    let sig = match ctx.posix_thread.dequeue_signal(&mask) {
        Some(sig) => sig,
        None => do_sigtimedwait(&mask, timeout, ctx)?,
    };

    if siginfo != 0 {
        ctx.user_space().write_val(siginfo, &sig.to_info())?;
    }

    Ok(SyscallReturn::Return(sig.num().as_u8() as isize))
}

fn do_sigtimedwait(mask: &SigMask, timeout: Vaddr, ctx: &Context) -> Result<Box<dyn Signal>> {
    let timeout = if timeout != 0 {
        let time_spec: timespec_t = ctx.user_space().read_val(timeout)?;
        let duration = Duration::try_from(time_spec)?;
        Some(ManagedTimeout::new_with_manager(
            Timeout::After(duration),
            MonotonicClock::timer_manager(),
        ))
    } else {
        None
    };

    let old_mask = ctx.posix_thread.sig_mask().load(Ordering::Relaxed);
    let new_mask = old_mask & *mask;
    ctx.posix_thread
        .sig_mask()
        .store(new_mask, Ordering::Relaxed);

    let waiter = Waiter::new_pair().0;
    let res = waiter.pause_until_or_timeout(|| ctx.posix_thread.dequeue_signal(&mask), timeout);

    ctx.posix_thread
        .sig_mask()
        .store(old_mask, Ordering::Relaxed);

    res
}
