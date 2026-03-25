// SPDX-License-Identifier: MPL-2.0

use ostd::mm::VmIo;

use super::{
    SyscallReturn,
    sched_setscheduler::{check_sched_change_perm, get_sched_target_info},
};
use crate::{prelude::*, sched::SchedPolicy, thread::Tid};

pub fn sys_sched_setparam(tid: Tid, addr: Vaddr, ctx: &Context) -> Result<SyscallReturn> {
    if addr == 0 {
        return_errno_with_message!(Errno::EINVAL, "invalid user space address");
    }

    let prio: i32 = ctx.user_space().read_val(addr)?;
    let target_info = get_sched_target_info(tid, ctx)?;
    let new_policy = match target_info.old_policy {
        SchedPolicy::RealTime { rt_policy, .. } => SchedPolicy::RealTime {
            rt_prio: u8::try_from(prio)
                .ok()
                .and_then(|p| p.try_into().ok())
                .ok_or_else(|| Error::with_message(Errno::EINVAL, "invalid scheduling priority"))?,
            rt_policy,
        },
        _ if prio != 0 => {
            return_errno_with_message!(Errno::EINVAL, "invalid scheduling priority")
        }
        policy => policy,
    };
    check_sched_change_perm(target_info, new_policy)?;
    if tid == 0 {
        ctx.thread.sched_attr().set_policy(new_policy);
    } else {
        let thread = crate::process::posix_thread::thread_table::get_thread(tid)
            .ok_or_else(|| Error::with_message(Errno::ESRCH, "the target thread does not exist"))?;
        thread.sched_attr().set_policy(new_policy);
    }

    Ok(SyscallReturn::Return(0))
}
