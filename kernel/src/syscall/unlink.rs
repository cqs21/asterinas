// SPDX-License-Identifier: MPL-2.0

use super::SyscallReturn;
use crate::{
    fs::{
        file::file_table::RawFileDesc,
        vfs::path::{AT_FDCWD, FsPath, SplitPath},
    },
    prelude::*,
    syscall::constants::MAX_FILENAME_LEN,
};

pub fn sys_unlinkat(
    dirfd: RawFileDesc,
    path_addr: Vaddr,
    flags: u32,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let flags =
        UnlinkFlags::from_bits(flags).ok_or(Error::with_message(Errno::EINVAL, "invalid flags"))?;
    if flags.contains(UnlinkFlags::AT_REMOVEDIR) {
        return super::rmdir::sys_rmdirat(dirfd, path_addr, ctx);
    }

    let path_name = ctx.user_space().read_cstring(path_addr, MAX_FILENAME_LEN)?;
    debug!("dirfd = {}, path = {:?}", dirfd, path_name);

    let path_name = path_name.to_string_lossy();
    let (dir_path, name) = {
        let fs_path = FsPath::from_fd_and_path(dirfd, &path_name)?;
        let fs_ref = ctx.thread_local.borrow_fs();
        let path_resolver = fs_ref.resolver().read();

        // Linux resolves the full unlink path before invoking the directory operation.
        // This preserves path-walk errors for trailing slash and dot components, such as
        // `file/`, `file/.`, and `file/..`, which should fail with `ENOTDIR` instead of
        // being reduced to unlinking the parent directory entry.
        path_resolver.lookup_unresolved_no_follow(&fs_path)?;

        let (parent_path_name, target_name) = path_name.split_dirname_and_basename()?;
        let parent_fs_path = FsPath::from_fd_and_path(dirfd, parent_path_name)?;
        (path_resolver.lookup(&parent_fs_path)?, target_name)
    };

    dir_path.unlink(name)?;
    Ok(SyscallReturn::Return(0))
}

pub fn sys_unlink(path_addr: Vaddr, ctx: &Context) -> Result<SyscallReturn> {
    self::sys_unlinkat(AT_FDCWD, path_addr, 0, ctx)
}

bitflags::bitflags! {
    struct UnlinkFlags: u32 {
        const AT_REMOVEDIR = 0x200;
    }
}
