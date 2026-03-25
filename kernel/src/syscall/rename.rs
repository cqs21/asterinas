// SPDX-License-Identifier: MPL-2.0

use alloc::format;

use super::SyscallReturn;
use crate::{
    fs::{
        file::{InodeType, file_table::FileDesc},
        vfs::path::{AT_FDCWD, FsPath, Path, PathResolver, SplitPath},
    },
    prelude::*,
    syscall::constants::MAX_FILENAME_LEN,
};

pub fn sys_renameat2(
    old_dirfd: FileDesc,
    old_path_addr: Vaddr,
    new_dirfd: FileDesc,
    new_path_addr: Vaddr,
    flags: u32,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let user_space = ctx.user_space();
    let old_path_name = user_space.read_cstring(old_path_addr, MAX_FILENAME_LEN)?;
    let new_path_name = user_space.read_cstring(new_path_addr, MAX_FILENAME_LEN)?;
    debug!(
        "old_dirfd = {}, old_path = {:?}, new_dirfd = {}, new_path = {:?}",
        old_dirfd, old_path_name, new_dirfd, new_path_name
    );
    let Some(flags) = Flags::from_bits(flags) else {
        return_errno_with_message!(Errno::EINVAL, "invalid flags");
    };
    if flags.intersects(Flags::WHITEOUT) || flags.contains(Flags::NOREPLACE | Flags::EXCHANGE) {
        return_errno_with_message!(Errno::EINVAL, "invalid renameat2 flag combination");
    }

    let fs_ref = ctx.thread_local.borrow_fs();
    let path_resolver = fs_ref.resolver().read();

    let old_path_name = old_path_name.to_string_lossy();
    let (old_parent_path, old_name) = {
        let (old_parent_path_name, old_name) = old_path_name.split_dirname_and_basename()?;
        let old_fs_path = FsPath::from_fd_and_path(old_dirfd, old_parent_path_name)?;
        (path_resolver.lookup(&old_fs_path)?, old_name)
    };
    let old_path = path_resolver.lookup_at_path(&old_parent_path, old_name)?;
    if old_path.type_() != InodeType::Dir && old_path_name.ends_with('/') {
        return_errno_with_message!(Errno::ENOTDIR, "the old path is not a directory");
    }

    let new_path_name = new_path_name.to_string_lossy();
    let (new_parent_path, new_name) = {
        if old_path.type_() != InodeType::Dir && new_path_name.ends_with('/') {
            return_errno_with_message!(Errno::EISDIR, "the new path is a directory");
        }
        let (new_parent_path_name, new_name) = new_path_name.split_dirname_and_basename()?;
        let new_parent_fs_path = FsPath::from_fd_and_path(new_dirfd, new_parent_path_name)?;
        (
            path_resolver.lookup(&new_parent_fs_path)?,
            new_name.to_string(),
        )
    };

    if old_path.type_() == InodeType::Dir && new_parent_path.is_equal_or_descendant_of(&old_path) {
        return_errno_with_message!(
            Errno::EINVAL,
            "the new path is inside the old directory or its subtree"
        );
    }

    if flags.contains(Flags::EXCHANGE) {
        let new_path = path_resolver.lookup_at_path(&new_parent_path, &new_name)?;
        if new_path.type_() == InodeType::Dir
            && old_parent_path.is_equal_or_descendant_of(&new_path)
        {
            return_errno_with_message!(
                Errno::EINVAL,
                "the old path is inside the new directory or its subtree"
            );
        }
        exchange_paths(
            &path_resolver,
            &old_parent_path,
            old_name,
            &new_parent_path,
            &new_name,
        )?;
    } else {
        if flags.contains(Flags::NOREPLACE) {
            match path_resolver.lookup_at_path(&new_parent_path, &new_name) {
                Ok(_) => return_errno_with_message!(Errno::EEXIST, "the new path already exists"),
                Err(err) if err.error() == Errno::ENOENT => {}
                Err(err) => return Err(err),
            }
        }
        old_parent_path.rename(old_name, &new_parent_path, &new_name)?;
    }

    Ok(SyscallReturn::Return(0))
}

pub fn sys_renameat(
    old_dirfd: FileDesc,
    old_path_addr: Vaddr,
    new_dirfd: FileDesc,
    new_path_addr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    self::sys_renameat2(old_dirfd, old_path_addr, new_dirfd, new_path_addr, 0, ctx)
}

pub fn sys_rename(
    old_path_addr: Vaddr,
    new_path_addr: Vaddr,
    ctx: &Context,
) -> Result<SyscallReturn> {
    self::sys_renameat2(AT_FDCWD, old_path_addr, AT_FDCWD, new_path_addr, 0, ctx)
}

bitflags! {
    /// Flags used in the `renameat2` system call.
    ///
    /// Reference: <https://elixir.bootlin.com/linux/v6.16.3/source/include/uapi/linux/fcntl.h#L140-L143>.
    struct Flags: u32 {
        const NOREPLACE = 1 << 0;
        const EXCHANGE  = 1 << 1;
        const WHITEOUT  = 1 << 2;
    }
}

fn exchange_paths(
    path_resolver: &PathResolver,
    old_parent_path: &Path,
    old_name: &str,
    new_parent_path: &Path,
    new_name: &str,
) -> Result<()> {
    if old_parent_path == new_parent_path && old_name == new_name {
        return Ok(());
    }

    let temp_name = create_exchange_temp_name(path_resolver, old_parent_path)?;
    old_parent_path.rename(old_name, old_parent_path, &temp_name)?;

    let exchange_result = (|| {
        new_parent_path.rename(new_name, old_parent_path, old_name)?;
        old_parent_path.rename(&temp_name, new_parent_path, new_name)
    })();
    if exchange_result.is_err() {
        let _ = old_parent_path.rename(&temp_name, old_parent_path, old_name);
    }

    exchange_result
}

fn create_exchange_temp_name(path_resolver: &PathResolver, parent_path: &Path) -> Result<String> {
    for suffix in 0..1024 {
        let candidate = format!(".renameat2-{}", suffix);
        match path_resolver.lookup_at_path(parent_path, &candidate) {
            Ok(_) => continue,
            Err(err) if err.error() == Errno::ENOENT => return Ok(candidate),
            Err(err) => return Err(err),
        }
    }

    return_errno_with_message!(
        Errno::EEXIST,
        "failed to allocate a temporary exchange name"
    );
}
