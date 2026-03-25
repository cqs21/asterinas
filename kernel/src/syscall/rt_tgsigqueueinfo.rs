// SPDX-License-Identifier: MPL-2.0

use ostd::mm::VmIo;

use super::SyscallReturn;
use crate::{
    prelude::*,
    process::{
        Pid,
        signal::{
            c_types::siginfo_t,
            constants::SI_TKILL,
            sig_num::SigNum,
            signals::{Signal, raw::RawSignal},
        },
        tgkill,
    },
    thread::Tid,
};

pub fn sys_rt_tgsigqueueinfo(
    tgid: Pid,
    tid: Tid,
    sig_num: u64,
    info_ptr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let sig_num = if sig_num == 0 {
        None
    } else {
        Some(SigNum::try_from(sig_num as u8)?)
    };
    debug!(
        "tgid = {}, tid = {}, sig_num = {:?}, info_ptr = {:#x}",
        tgid, tid, sig_num, info_ptr
    );

    let signal = if let Some(sig_num) = sig_num {
        let siginfo = read_siginfo_from_user(info_ptr, sig_num, ctx)?;
        validate_si_code(siginfo.si_code, tgid, ctx)?;
        Some(Box::new(RawSignal::new(siginfo)) as Box<dyn Signal>)
    } else {
        None
    };

    tgkill(tid, tgid, signal, ctx)?;
    Ok(SyscallReturn::Return(0))
}

fn read_siginfo_from_user(info_ptr: Vaddr, sig_num: SigNum, ctx: &Context) -> Result<siginfo_t> {
    let mut siginfo = ctx.user_space().read_val::<siginfo_t>(info_ptr)?;
    siginfo.si_signo = sig_num.as_u8() as i32;
    Ok(siginfo)
}

fn validate_si_code(si_code: i32, target_tgid: Pid, ctx: &Context) -> Result<()> {
    if target_tgid != ctx.process.pid() && (si_code >= 0 || si_code == SI_TKILL) {
        return_errno_with_message!(
            Errno::EPERM,
            "signals with nonnegative si_code or SI_TKILL require self-targeting"
        );
    }

    Ok(())
}
