// SPDX-License-Identifier: MPL-2.0

use super::SyscallReturn;
use crate::{
    fs,
    fs::{
        file::file_table::{FileDesc, get_file_fast},
        utils::PATH_MAX,
        vfs::{
            inode_ext::InodeExt,
            path::{AT_FDCWD, FsPath},
        },
    },
    prelude::*,
    process::{Process, ResourceType},
};

pub fn sys_ftruncate(fd: FileDesc, len: isize, ctx: &Context) -> Result<SyscallReturn> {
    debug!("fd = {}, length = {}", fd, len);

    check_length(len, ctx)?;

    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    file.resize(len as usize)?;
    fs::vfs::notify::on_change(file.path());
    Ok(SyscallReturn::Return(0))
}

pub fn sys_truncate(path_ptr: Vaddr, len: isize, ctx: &Context) -> Result<SyscallReturn> {
    let path_name = ctx.user_space().read_cstring(path_ptr, PATH_MAX)?;
    debug!("path = {:?}, length = {}", path_name, len);

    check_length(len, ctx)?;

    let dir_path = {
        let path_name = path_name.to_string_lossy();
        let fs_path = FsPath::from_fd_and_path(AT_FDCWD, &path_name)?;
        ctx.thread_local
            .borrow_fs()
            .resolver()
            .read()
            .lookup(&fs_path)?
    };

    if let Some(lock_context) = dir_path.inode().fs_lock_context() {
        let requester_pid = Process::current().map(|process| process.pid());
        lock_context.notify_lease_break_for_truncate(requester_pid);
    }

    dir_path.resize(len as usize)?;
    fs::vfs::notify::on_change(&dir_path);
    Ok(SyscallReturn::Return(0))
}

#[inline]
fn check_length(len: isize, ctx: &Context) -> Result<()> {
    if len < 0 {
        return_errno_with_message!(Errno::EINVAL, "length is negative");
    }

    let max_file_size = {
        let resource_limits = ctx.process.resource_limits();
        resource_limits
            .get_rlimit(ResourceType::RLIMIT_FSIZE)
            .get_cur() as usize
    };
    if len as usize > max_file_size {
        return_errno_with_message!(Errno::EFBIG, "length is larger than the maximum file size");
    }
    Ok(())
}
