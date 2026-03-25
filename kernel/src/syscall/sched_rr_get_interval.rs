// SPDX-License-Identifier: MPL-2.0

use core::time::Duration;

use ostd::mm::VmIo;

use super::{SyscallReturn, sched_getattr::access_sched_attr_with};
use crate::{
    prelude::*,
    sched::{MIN_PERIOD_NS, SchedPolicy},
    thread::Tid,
    time::timespec_t,
};

pub fn sys_sched_rr_get_interval(
    tid: Tid,
    interval_addr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let interval_ns = access_sched_attr_with(tid, ctx, |attr| Ok(rr_interval_ns(attr.policy())))?;
    let interval = timespec_t::from(Duration::from_nanos(interval_ns));
    ctx.user_space().write_val(interval_addr, &interval)?;

    Ok(SyscallReturn::Return(0))
}

fn rr_interval_ns(policy: SchedPolicy) -> u64 {
    match policy {
        SchedPolicy::Deadline { .. } => 0,
        SchedPolicy::RealTime { rt_policy, .. } => rt_policy.rr_interval_ns(),
        SchedPolicy::Fair(_) | SchedPolicy::Idle => MIN_PERIOD_NS,
        SchedPolicy::Stop => 0,
    }
}
