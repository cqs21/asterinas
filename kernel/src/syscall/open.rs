// SPDX-License-Identifier: MPL-2.0

use core::mem::size_of;

use ostd::mm::VmIo;

use super::SyscallReturn;
use crate::{
    fs,
    fs::{
        file::{
            AccessMode, CreationFlags, FileLike, InodeHandle, InodeMode, InodeType, OpenArgs,
            StatusFlags,
            file_table::{FdFlags, FileDesc},
        },
        vfs::path::{AT_FDCWD, FsPath, LookupResult, Openat2ResolveFlags, PathResolver},
    },
    prelude::*,
    process::ResourceType,
    syscall::constants::MAX_FILENAME_LEN,
};

pub fn sys_openat(
    dirfd: FileDesc,
    path_addr: Vaddr,
    flags: u32,
    mode: u16,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let path = ctx.user_space().read_cstring(path_addr, MAX_FILENAME_LEN)?;
    debug!(
        "dirfd = {}, path = {:?}, flags = {}, mode = {}",
        dirfd, path, flags, mode
    );

    let path = path.to_string_lossy();
    let file_handle = open_with_flags(
        dirfd,
        path.as_ref(),
        flags,
        mode,
        Openat2ResolveFlags::empty(),
        ctx,
    )
    .map_err(|err| match err.error() {
        Errno::EINTR => Error::new(Errno::ERESTARTSYS),
        _ => err,
    })?;
    install_opened_file(file_handle, flags, ctx)
}

pub fn sys_openat2(
    dirfd: FileDesc,
    path_addr: Vaddr,
    how_addr: Vaddr,
    size: usize,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let path = ctx.user_space().read_cstring(path_addr, MAX_FILENAME_LEN)?;
    let how = read_open_how(how_addr, size, ctx)?;
    debug!(
        "dirfd = {}, path = {:?}, flags = {:#x}, mode = {:#o}, resolve = {:?}",
        dirfd,
        path,
        how.flags,
        how.mode.bits(),
        how.resolve
    );

    let path = path.to_string_lossy();
    let file_handle = open_with_flags(
        dirfd,
        path.as_ref(),
        how.flags,
        how.mode.bits(),
        how.resolve,
        ctx,
    )
    .map_err(|err| match err.error() {
        Errno::EINTR => Error::new(Errno::ERESTARTSYS),
        _ => err,
    })?;
    install_opened_file(file_handle, how.flags, ctx)
}

pub fn sys_open(path_addr: Vaddr, flags: u32, mode: u16, ctx: &Context) -> Result<SyscallReturn> {
    self::sys_openat(AT_FDCWD, path_addr, flags, mode, ctx)
}

pub fn sys_creat(path_addr: Vaddr, mode: u16, ctx: &Context) -> Result<SyscallReturn> {
    let flags =
        AccessMode::O_WRONLY as u32 | CreationFlags::O_CREAT.bits() | CreationFlags::O_TRUNC.bits();
    self::sys_openat(AT_FDCWD, path_addr, flags, mode, ctx)
}

fn do_open(
    path_resolver: &PathResolver,
    fs_path: &FsPath,
    flags: u32,
    mode: InodeMode,
) -> Result<Arc<dyn FileLike>> {
    let open_args = OpenArgs::from_flags_and_mode(flags, mode)?;

    let lookup_res = if open_args.follow_tail_link() {
        path_resolver.lookup_unresolved(fs_path)?
    } else {
        path_resolver.lookup_unresolved_no_follow(fs_path)?
    };

    let file_handle: Arc<dyn FileLike> = match lookup_res {
        LookupResult::Resolved(path) => Arc::new(path.open(open_args)?),
        LookupResult::AtParent(result) => {
            if !open_args.creation_flags.contains(CreationFlags::O_CREAT)
                || open_args.status_flags.contains(StatusFlags::O_PATH)
            {
                return_errno_with_message!(Errno::ENOENT, "the file does not exist");
            }
            if open_args
                .creation_flags
                .contains(CreationFlags::O_DIRECTORY)
            {
                return_errno_with_message!(
                    Errno::EINVAL,
                    "O_CREAT and O_DIRECTORY cannot be specified together"
                );
            }
            if result.target_is_dir() {
                return_errno_with_message!(
                    Errno::EISDIR,
                    "O_CREAT is specified but the file is a directory"
                );
            }

            let (parent, tail_name) = result.into_parent_and_basename();
            let new_path =
                parent.new_fs_child(&tail_name, InodeType::File, open_args.inode_mode)?;
            fs::vfs::notify::on_create(&parent, || tail_name.clone());

            // Don't check access mode for newly created file.
            Arc::new(InodeHandle::new_unchecked_access(
                new_path,
                open_args.access_mode,
                open_args.status_flags,
            )?)
        }
    };

    Ok(file_handle)
}

fn open_with_flags(
    dirfd: FileDesc,
    path: &str,
    flags: u32,
    mode: u16,
    resolve_flags: Openat2ResolveFlags,
    ctx: &Context,
) -> Result<Arc<dyn FileLike>> {
    let fs_ref = ctx.thread_local.borrow_fs();
    let mask_mode = mode & !fs_ref.umask().get();
    let path_resolver = fs_ref.resolver().read();

    if resolve_flags.is_empty() {
        let fs_path = FsPath::from_fd_and_path(dirfd, path)?;
        do_open(
            &path_resolver,
            &fs_path,
            flags,
            InodeMode::from_bits_truncate(mask_mode),
        )
    } else {
        do_openat2(
            &path_resolver,
            dirfd,
            path,
            flags,
            InodeMode::from_bits_truncate(mask_mode),
            resolve_flags,
        )
    }
}

fn do_openat2(
    path_resolver: &PathResolver,
    dirfd: FileDesc,
    path: &str,
    flags: u32,
    mode: InodeMode,
    resolve_flags: Openat2ResolveFlags,
) -> Result<Arc<dyn FileLike>> {
    let open_args = OpenArgs::from_flags_and_mode(flags, mode)?;
    let lookup_res =
        path_resolver.lookup_openat2(dirfd, path, open_args.follow_tail_link(), resolve_flags)?;
    finish_open_from_lookup(lookup_res, open_args)
}

fn finish_open_from_lookup(
    lookup_res: LookupResult,
    open_args: OpenArgs,
) -> Result<Arc<dyn FileLike>> {
    let file_handle: Arc<dyn FileLike> = match lookup_res {
        LookupResult::Resolved(path) => Arc::new(path.open(open_args)?),
        LookupResult::AtParent(result) => {
            if !open_args.creation_flags.contains(CreationFlags::O_CREAT)
                || open_args.status_flags.contains(StatusFlags::O_PATH)
            {
                return_errno_with_message!(Errno::ENOENT, "the file does not exist");
            }
            if open_args
                .creation_flags
                .contains(CreationFlags::O_DIRECTORY)
            {
                return_errno_with_message!(
                    Errno::EINVAL,
                    "O_CREAT and O_DIRECTORY cannot be specified together"
                );
            }
            if result.target_is_dir() {
                return_errno_with_message!(
                    Errno::EISDIR,
                    "O_CREAT is specified but the file is a directory"
                );
            }

            let (parent, tail_name) = result.into_parent_and_basename();
            let new_path =
                parent.new_fs_child(&tail_name, InodeType::File, open_args.inode_mode)?;
            fs::vfs::notify::on_create(&parent, || tail_name.clone());

            Arc::new(InodeHandle::new_unchecked_access(
                new_path,
                open_args.access_mode,
                open_args.status_flags,
            )?)
        }
    };

    Ok(file_handle)
}

fn install_opened_file(
    file_handle: Arc<dyn FileLike>,
    flags: u32,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let fd = {
        let file_table = ctx.thread_local.borrow_file_table();
        let mut file_table_locked = file_table.unwrap().write();
        let fd_flags =
            if CreationFlags::from_bits_truncate(flags).contains(CreationFlags::O_CLOEXEC) {
                FdFlags::CLOEXEC
            } else {
                FdFlags::empty()
            };
        let max_fd_exclusive = ctx
            .process
            .resource_limits()
            .get_rlimit(ResourceType::RLIMIT_NOFILE)
            .get_cur()
            .min(i32::MAX as u64 + 1) as usize;
        file_table_locked.insert_with_limit(file_handle.clone(), fd_flags, max_fd_exclusive)?
    };
    fs::vfs::notify::on_open(&file_handle);
    Ok(SyscallReturn::Return(fd as _))
}

fn read_open_how(how_addr: Vaddr, size: usize, ctx: &Context) -> Result<OpenHowArgs> {
    if size < size_of::<OpenHow>() {
        return_errno_with_message!(Errno::EINVAL, "open_how is too small");
    }

    let user_space = ctx.user_space();
    let how: OpenHow = user_space.read_val(how_addr)?;

    if size > size_of::<OpenHow>() {
        let extra_addr = how_addr
            .checked_add(size_of::<OpenHow>())
            .ok_or_else(|| Error::with_message(Errno::EFAULT, "open_how pointer overflow"))?;
        let mut extra = vec![0u8; size - size_of::<OpenHow>()];
        user_space.read_bytes(extra_addr, &mut extra)?;
        if extra.iter().any(|byte| *byte != 0) {
            return_errno_with_message!(Errno::E2BIG, "open_how contains non-zero extension data");
        }
    }

    if how.flags > u32::MAX as u64 {
        return_errno_with_message!(Errno::EINVAL, "invalid open flags");
    }
    let flags = how.flags as u32;
    validate_openat2_flags(flags)?;

    let creation_flags = CreationFlags::from_bits_truncate(flags);
    if creation_flags.intersects(CreationFlags::O_CREAT | CreationFlags::O_TMPFILE) {
        if how.mode & !(InodeMode::all().bits() as u64) != 0 {
            return_errno_with_message!(Errno::EINVAL, "invalid file mode");
        }
    } else if how.mode != 0 {
        return_errno_with_message!(Errno::EINVAL, "mode requires O_CREAT or O_TMPFILE");
    }

    let resolve = Openat2ResolveFlags::from_bits(how.resolve)
        .ok_or_else(|| Error::with_message(Errno::EINVAL, "invalid resolve flags"))?;

    Ok(OpenHowArgs {
        flags,
        mode: InodeMode::from_bits_truncate(how.mode as u16),
        resolve,
    })
}

fn validate_openat2_flags(flags: u32) -> Result<()> {
    let supported_bits = 0b11 | CreationFlags::all().bits() | StatusFlags::all().bits();
    if flags & !supported_bits != 0 {
        return_errno_with_message!(Errno::EINVAL, "unsupported open flags");
    }

    let _ = AccessMode::from_u32(flags)?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct OpenHowArgs {
    flags: u32,
    mode: InodeMode,
    resolve: Openat2ResolveFlags,
}

#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}
