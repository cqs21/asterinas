// SPDX-License-Identifier: MPL-2.0

use super::SyscallReturn;
use crate::prelude::*;

pub fn sys_set_tid_address(tidptr: Vaddr, ctx: &Context) -> Result<SyscallReturn> {
    debug!("tidptr = 0x{:x}", tidptr);

    ctx.thread_local.set_child_tid().set(tidptr);

    let tid = ctx.posix_thread.tid();
    Ok(SyscallReturn::Return(tid as _))
}
