// SPDX-License-Identifier: MPL-2.0

use ostd::mm::VmIo;

use super::{
    SyscallReturn,
    sched_get_priority_max::rt_to_static,
    sched_getattr::{LinuxSchedAttr, access_sched_attr_with},
};
use crate::{
    prelude::*,
    process::{
        ResourceType,
        credentials::capabilities::CapSet,
        posix_thread::{AsPosixThread, thread_table},
    },
    sched::SchedPolicy,
    thread::Tid,
};

#[derive(Clone, Copy)]
pub(super) struct SchedTargetInfo {
    pub(super) old_policy: SchedPolicy,
    rtprio_limit: u64,
    same_owner: bool,
    has_sys_nice: bool,
}

pub fn sys_sched_setscheduler(
    tid: Tid,
    policy: i32,
    addr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    if addr == 0 {
        return_errno_with_message!(Errno::EINVAL, "invalid user space address");
    }

    let prio = ctx.user_space().read_val(addr)?;
    let target_info = get_sched_target_info(tid, ctx)?;

    let attr = LinuxSchedAttr {
        sched_policy: policy as u32,
        sched_priority: prio,
        ..Default::default()
    };

    let policy = attr.try_into()?;
    check_sched_change_perm(target_info, policy)?;
    access_sched_attr_with(tid, ctx, |attr| {
        attr.set_policy(policy);
        Ok(())
    })?;

    Ok(SyscallReturn::Return(0))
}

pub(super) fn get_sched_target_info(tid: Tid, ctx: &Context) -> Result<SchedTargetInfo> {
    if tid.cast_signed() < 0 {
        return_errno_with_message!(Errno::EINVAL, "all negative TIDs are not valid");
    }

    if tid == 0 {
        return Ok(SchedTargetInfo {
            old_policy: ctx.thread.sched_attr().policy(),
            rtprio_limit: ctx
                .process
                .resource_limits()
                .get_rlimit(ResourceType::RLIMIT_RTPRIO)
                .get_cur(),
            same_owner: true,
            has_sys_nice: ctx
                .process
                .user_ns()
                .lock()
                .check_cap(CapSet::SYS_NICE, ctx.posix_thread)
                .is_ok(),
        });
    }

    let Some(thread) = thread_table::get_thread(tid) else {
        return_errno_with_message!(Errno::ESRCH, "the target thread does not exist");
    };
    let target_posix_thread = thread.as_posix_thread().unwrap();
    let target_process = target_posix_thread.process();

    let current_cred = ctx.posix_thread.credentials();
    let target_cred = target_posix_thread.credentials();

    Ok(SchedTargetInfo {
        old_policy: thread.sched_attr().policy(),
        rtprio_limit: target_process
            .resource_limits()
            .get_rlimit(ResourceType::RLIMIT_RTPRIO)
            .get_cur(),
        same_owner: current_cred.euid() == target_cred.ruid()
            || current_cred.euid() == target_cred.euid(),
        has_sys_nice: target_process
            .user_ns()
            .lock()
            .check_cap(CapSet::SYS_NICE, ctx.posix_thread)
            .is_ok(),
    })
}

pub(super) fn check_sched_change_perm(
    target_info: SchedTargetInfo,
    new_policy: SchedPolicy,
) -> Result<()> {
    if target_info.has_sys_nice {
        return Ok(());
    }

    if !target_info.same_owner {
        return_errno_with_message!(
            Errno::EPERM,
            "changing the scheduling policy of another user's thread is not allowed"
        );
    }

    let SchedPolicy::RealTime {
        rt_prio: new_rt_prio,
        ..
    } = new_policy
    else {
        return Ok(());
    };

    if !matches!(target_info.old_policy, SchedPolicy::RealTime { .. })
        && target_info.rtprio_limit == 0
    {
        return_errno_with_message!(
            Errno::EPERM,
            "switching to a real-time scheduling policy requires CAP_SYS_NICE or RLIMIT_RTPRIO"
        );
    }

    let old_user_priority = match target_info.old_policy {
        SchedPolicy::RealTime { rt_prio, .. } => u64::from(rt_to_static(rt_prio)),
        _ => 0,
    };
    let new_user_priority = u64::from(rt_to_static(new_rt_prio));

    if new_user_priority > old_user_priority && new_user_priority > target_info.rtprio_limit {
        return_errno_with_message!(
            Errno::EPERM,
            "raising the real-time priority beyond RLIMIT_RTPRIO requires CAP_SYS_NICE"
        );
    }

    Ok(())
}
