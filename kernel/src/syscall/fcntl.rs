// SPDX-License-Identifier: MPL-2.0

use ostd::mm::VmIo;

use super::SyscallReturn;
use crate::{
    fs::{
        file::{
            FileLike, StatusFlags,
            file_table::{FdFlags, FileAsyncOwner, FileDesc, WithFileTable, get_file_fast},
        },
        ramfs::memfd::{FileSeals, MemfdInodeHandle},
        vfs::range_lock::{FileRange, OFFSET_MAX, RangeLockItem, RangeLockType},
    },
    prelude::*,
    process::{
        Pgid, Pid, ResourceType,
        posix_thread::thread_table,
        process_table,
        signal::{
            constants::{SIGIO, SIGKILL, SIGSTOP},
            sig_num::SigNum,
        },
    },
    thread::Tid,
};

pub fn sys_fcntl(fd: FileDesc, cmd: i32, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let fcntl_cmd = FcntlCmd::try_from(cmd)?;
    debug!("fd = {}, cmd = {:?}, arg = {}", fd, fcntl_cmd, arg);
    match fcntl_cmd {
        FcntlCmd::F_DUPFD => handle_dupfd(fd, arg, FdFlags::empty(), ctx),
        FcntlCmd::F_DUPFD_CLOEXEC => handle_dupfd(fd, arg, FdFlags::CLOEXEC, ctx),
        FcntlCmd::F_GETFD => handle_getfd(fd, ctx),
        FcntlCmd::F_SETFD => handle_setfd(fd, arg, ctx),
        FcntlCmd::F_GETFL => handle_getfl(fd, ctx),
        FcntlCmd::F_SETFL => handle_setfl(fd, arg, ctx),
        FcntlCmd::F_GETLK => handle_getlk(fd, arg, ctx),
        FcntlCmd::F_SETLK => handle_setlk(fd, arg, true, ctx),
        FcntlCmd::F_SETLKW => handle_setlk(fd, arg, false, ctx).map_err(|err| match err.error() {
            Errno::EINTR => Error::new(Errno::ERESTARTSYS),
            _ => err,
        }),
        FcntlCmd::F_SETLEASE => handle_setlease(fd, arg, ctx),
        FcntlCmd::F_GETLEASE => handle_getlease(fd, ctx),
        FcntlCmd::F_GETOWN => handle_getown(fd, ctx),
        FcntlCmd::F_SETOWN => handle_setown(fd, arg, ctx),
        FcntlCmd::F_SETSIG => handle_setsig(fd, arg, ctx),
        FcntlCmd::F_GETSIG => handle_getsig(fd, ctx),
        FcntlCmd::F_SETOWN_EX => handle_setown_ex(fd, arg, ctx),
        FcntlCmd::F_GETOWN_EX => handle_getown_ex(fd, arg, ctx),
        FcntlCmd::F_SETPIPE_SZ => handle_setpipe_sz(fd, arg, ctx),
        FcntlCmd::F_GETPIPE_SZ => handle_getpipe_sz(fd, ctx),
        FcntlCmd::F_ADD_SEALS => handle_addseal(fd, arg, ctx),
        FcntlCmd::F_GET_SEALS => handle_getseal(fd, ctx),
    }
}

fn handle_dupfd(fd: FileDesc, arg: u64, flags: FdFlags, ctx: &Context) -> Result<SyscallReturn> {
    let minimum_fd = i32::try_from(arg)
        .map_err(|_| Error::with_message(Errno::EINVAL, "invalid minimum fd value"))?;
    let max_fd_exclusive = ctx
        .process
        .resource_limits()
        .get_rlimit(ResourceType::RLIMIT_NOFILE)
        .get_cur()
        .min(i32::MAX as u64 + 1) as usize;

    if minimum_fd as usize >= max_fd_exclusive {
        return_errno_with_message!(Errno::EINVAL, "minimum fd exceeds RLIMIT_NOFILE");
    }

    let file_table = ctx.thread_local.borrow_file_table();
    let new_fd = file_table.unwrap().write().dup_ceil_with_limit(
        fd,
        minimum_fd as FileDesc,
        max_fd_exclusive,
        flags,
    )?;
    Ok(SyscallReturn::Return(new_fd as _))
}

fn handle_getfd(fd: FileDesc, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    file_table.read_with(|inner| {
        let fd_flags = inner.get_entry(fd)?.flags();
        Ok(SyscallReturn::Return(fd_flags.bits() as _))
    })
}

fn handle_setfd(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let flags = if arg > u64::from(u8::MAX) {
        return_errno_with_message!(Errno::EINVAL, "invalid fd flags");
    } else {
        FdFlags::from_bits(arg as u8).ok_or(Error::with_message(Errno::EINVAL, "invalid flags"))?
    };
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    file_table.read_with(|inner| {
        inner.get_entry(fd)?.set_flags(flags);
        Ok(SyscallReturn::Return(0))
    })
}

fn handle_getfl(fd: FileDesc, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    let status_flags = file.status_flags();
    let access_mode = file.access_mode();
    Ok(SyscallReturn::Return(
        (status_flags.bits() | access_mode as u32) as _,
    ))
}

fn handle_setfl(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    let valid_flags_mask = StatusFlags::O_APPEND
        | StatusFlags::O_ASYNC
        | StatusFlags::O_DIRECT
        | StatusFlags::O_NOATIME
        | StatusFlags::O_NONBLOCK;
    let mut status_flags = file.status_flags();
    status_flags.remove(valid_flags_mask);
    status_flags.insert(StatusFlags::from_bits_truncate(arg as _) & valid_flags_mask);
    file.set_status_flags(status_flags)?;
    Ok(SyscallReturn::Return(0))
}

fn handle_getlk(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    let lock_mut_ptr = arg as Vaddr;
    let mut lock_mut_c = ctx.user_space().read_val::<c_flock>(lock_mut_ptr)?;
    let lock_type = RangeLockType::try_from(lock_mut_c.l_type)?;
    if lock_type == RangeLockType::Unlock {
        return_errno_with_message!(Errno::EINVAL, "invalid flock type for getlk");
    }
    let mut lock = RangeLockItem::new(lock_type, from_c_flock_and_file(&lock_mut_c, &**file)?);
    let inode_file = file.as_inode_handle_or_err()?;
    lock = inode_file.test_range_lock(lock)?;
    lock_mut_c.copy_from_range_lock(&lock);
    ctx.user_space().write_val(lock_mut_ptr, &lock_mut_c)?;
    Ok(SyscallReturn::Return(0))
}

fn handle_setlk(
    fd: FileDesc,
    arg: u64,
    is_nonblocking: bool,
    ctx: &Context,
) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    let lock_mut_ptr = arg as Vaddr;
    let lock_mut_c = ctx.user_space().read_val::<c_flock>(lock_mut_ptr)?;
    let lock_type = RangeLockType::try_from(lock_mut_c.l_type)?;
    let lock = RangeLockItem::new(lock_type, from_c_flock_and_file(&lock_mut_c, &**file)?);
    let inode_file = file.as_inode_handle_or_err()?;
    inode_file.set_range_lock(&lock, is_nonblocking)?;
    Ok(SyscallReturn::Return(0))
}

fn handle_getown(fd: FileDesc, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    file_table.read_with(|inner| {
        let owner = inner.get_entry(fd)?.owner();
        let value = match owner {
            None => 0,
            Some(FileAsyncOwner::Thread(tid)) => tid as i32,
            Some(FileAsyncOwner::Process(pid)) => pid as i32,
            Some(FileAsyncOwner::ProcessGroup(pgid)) => -(pgid as i32),
        };
        Ok(SyscallReturn::Return(value as _))
    })
}

fn handle_setown(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let owner = file_async_owner_from_setown_arg(arg)?;

    let file_table = ctx.thread_local.borrow_file_table();
    let mut file_table_locked = file_table.unwrap().write();
    let file_entry = file_table_locked.get_entry_mut(fd)?;
    file_entry.set_owner(owner)?;
    Ok(SyscallReturn::Return(0))
}

fn handle_setsig(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let signal = signal_from_setsig_arg(arg)?;

    let file_table = ctx.thread_local.borrow_file_table();
    let mut file_table_locked = file_table.unwrap().write();
    let file_entry = file_table_locked.get_entry_mut(fd)?;
    file_entry.set_signal(signal)?;
    Ok(SyscallReturn::Return(0))
}

fn handle_getsig(fd: FileDesc, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    file_table.read_with(|inner| {
        let signal = inner
            .get_entry(fd)?
            .signal()
            .map_or(0, |signal| signal.as_u8());
        Ok(SyscallReturn::Return(signal as _))
    })
}

fn handle_setown_ex(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let owner_ex_ptr = arg as Vaddr;
    let owner_ex = ctx.user_space().read_val::<f_owner_ex>(owner_ex_ptr)?;
    let owner = owner_ex.try_to_async_owner()?;

    let file_table = ctx.thread_local.borrow_file_table();
    let mut file_table_locked = file_table.unwrap().write();
    let file_entry = file_table_locked.get_entry_mut(fd)?;
    file_entry.set_owner(owner)?;
    Ok(SyscallReturn::Return(0))
}

fn handle_getown_ex(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let owner_ex_ptr = arg as Vaddr;
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let owner_ex = file_table.read_with(|inner| {
        inner
            .get_entry(fd)
            .map(|entry| f_owner_ex::from_async_owner(entry.owner()))
    })?;
    ctx.user_space().write_val(owner_ex_ptr, &owner_ex)?;
    Ok(SyscallReturn::Return(0))
}

fn handle_setlease(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let lease_type = lease_type_from_arg(arg)?;

    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    file.as_inode_handle_or_err()?
        .set_lease(lease_type, ctx.process.pid())?;

    Ok(SyscallReturn::Return(0))
}

fn handle_getlease(fd: FileDesc, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    let lease_type = file.as_inode_handle_or_err()?.get_lease();

    Ok(SyscallReturn::Return(lease_type as _))
}

fn handle_setpipe_sz(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let requested_size = usize::try_from(arg)
        .map_err(|_| Error::with_message(Errno::EINVAL, "invalid pipe size"))?;
    if requested_size == 0 {
        return_errno_with_message!(Errno::EINVAL, "pipe size must be positive");
    }

    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    let inode_handle = file.as_inode_handle_or_err()?;
    let actual_size = inode_handle.set_pipe_capacity(requested_size)?;
    Ok(SyscallReturn::Return(actual_size as _))
}

fn handle_getpipe_sz(fd: FileDesc, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);
    let inode_handle = file.as_inode_handle_or_err()?;
    let pipe_size = inode_handle.pipe_capacity()?;
    Ok(SyscallReturn::Return(pipe_size as _))
}

fn handle_addseal(fd: FileDesc, arg: u64, ctx: &Context) -> Result<SyscallReturn> {
    let new_seals = FileSeals::from_bits(arg as u32)
        .ok_or_else(|| Error::with_message(Errno::EINVAL, "invalid seals"))?;

    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);

    file.as_inode_handle_or_err()?.add_seals(new_seals)?;

    Ok(SyscallReturn::Return(0))
}

fn handle_getseal(fd: FileDesc, ctx: &Context) -> Result<SyscallReturn> {
    let mut file_table = ctx.thread_local.borrow_file_table_mut();
    let file = get_file_fast!(&mut file_table, fd);

    let file_seals = file.as_inode_handle_or_err()?.get_seals()?;

    Ok(SyscallReturn::Return(file_seals.bits() as _))
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, TryFromInt)]
#[expect(non_camel_case_types)]
enum FcntlCmd {
    F_DUPFD = 0,
    F_GETFD = 1,
    F_SETFD = 2,
    F_GETFL = 3,
    F_SETFL = 4,
    F_GETLK = 5,
    F_SETLK = 6,
    F_SETLKW = 7,
    F_SETOWN = 8,
    F_GETOWN = 9,
    F_SETSIG = 10,
    F_GETSIG = 11,
    F_SETOWN_EX = 15,
    F_GETOWN_EX = 16,
    F_SETLEASE = 1024,
    F_GETLEASE = 1025,
    F_DUPFD_CLOEXEC = 1030,
    F_SETPIPE_SZ = 1031,
    F_GETPIPE_SZ = 1032,
    F_ADD_SEALS = 1033,
    F_GET_SEALS = 1034,
}

#[expect(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromInt)]
#[repr(i32)]
enum FOwnerType {
    F_OWNER_TID = 0,
    F_OWNER_PID = 1,
    F_OWNER_PGRP = 2,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
struct f_owner_ex {
    type_: i32,
    pid: i32,
}

impl f_owner_ex {
    fn from_async_owner(owner: Option<FileAsyncOwner>) -> Self {
        match owner {
            Some(FileAsyncOwner::Thread(tid)) => Self {
                type_: FOwnerType::F_OWNER_TID as i32,
                pid: tid as i32,
            },
            Some(FileAsyncOwner::Process(pid)) => Self {
                type_: FOwnerType::F_OWNER_PID as i32,
                pid: pid as i32,
            },
            Some(FileAsyncOwner::ProcessGroup(pgid)) => Self {
                type_: FOwnerType::F_OWNER_PGRP as i32,
                pid: pgid as i32,
            },
            None => Self {
                type_: FOwnerType::F_OWNER_PID as i32,
                pid: 0,
            },
        }
    }

    fn try_to_async_owner(&self) -> Result<Option<FileAsyncOwner>> {
        if self.pid == 0 {
            return Ok(None);
        }

        let owner_type = FOwnerType::try_from(self.type_)?;
        let pid = u32::try_from(self.pid)
            .map_err(|_| Error::with_message(Errno::EINVAL, "invalid owner id"))?;

        match owner_type {
            FOwnerType::F_OWNER_TID => {
                let tid = Tid::try_from(pid)
                    .map_err(|_| Error::with_message(Errno::EINVAL, "invalid thread id"))?;
                if thread_table::get_thread(tid).is_none() {
                    return_errno_with_message!(
                        Errno::ESRCH,
                        "cannot set owner with an invalid tid"
                    );
                }
                Ok(Some(FileAsyncOwner::Thread(tid)))
            }
            FOwnerType::F_OWNER_PID => {
                let process_pid = Pid::try_from(pid)
                    .map_err(|_| Error::with_message(Errno::EINVAL, "invalid process id"))?;
                if process_table::get_process(process_pid).is_none() {
                    return_errno_with_message!(
                        Errno::ESRCH,
                        "cannot set owner with an invalid pid"
                    );
                }
                Ok(Some(FileAsyncOwner::Process(process_pid)))
            }
            FOwnerType::F_OWNER_PGRP => {
                let pgid = Pgid::try_from(pid)
                    .map_err(|_| Error::with_message(Errno::EINVAL, "invalid process group id"))?;
                if process_table::get_process_group(&pgid).is_none() {
                    return_errno_with_message!(
                        Errno::ESRCH,
                        "cannot set owner with an invalid pgid"
                    );
                }
                Ok(Some(FileAsyncOwner::ProcessGroup(pgid)))
            }
        }
    }
}

#[expect(non_camel_case_types)]
pub type off_t = i64;

#[expect(non_camel_case_types)]
#[derive(Debug, Copy, Clone, TryFromInt)]
#[repr(u16)]
pub enum RangeLockWhence {
    SEEK_SET = 0,
    SEEK_CUR = 1,
    SEEK_END = 2,
}

/// C struct for a file range lock in Libc
#[repr(C)]
#[padding_struct]
#[derive(Debug, Copy, Clone, Pod)]
pub struct c_flock {
    /// Type of lock: F_RDLCK, F_WRLCK, or F_UNLCK
    pub l_type: u16,
    /// Where `l_start' is relative to
    pub l_whence: u16,
    /// Offset where the lock begins
    pub l_start: off_t,
    /// Size of the locked area, 0 means until EOF
    pub l_len: off_t,
    /// Process holding the lock
    pub l_pid: Pid,
}

impl c_flock {
    pub fn copy_from_range_lock(&mut self, lock: &RangeLockItem) {
        self.l_type = lock.type_() as u16;
        if RangeLockType::Unlock != lock.type_() {
            self.l_whence = RangeLockWhence::SEEK_SET as u16;
            self.l_start = lock.start() as off_t;
            self.l_len = if lock.end() == OFFSET_MAX {
                0
            } else {
                lock.range().len() as off_t
            };
            self.l_pid = lock.owner();
        }
    }
}

/// Create the file range through C flock and opened file reference
fn from_c_flock_and_file(lock: &c_flock, file: &dyn FileLike) -> Result<FileRange> {
    let start = {
        let whence = RangeLockWhence::try_from(lock.l_whence)?;
        match whence {
            RangeLockWhence::SEEK_SET => lock.l_start,
            RangeLockWhence::SEEK_CUR => (file.as_inode_handle_or_err()?.offset() as off_t)
                .checked_add(lock.l_start)
                .ok_or(Error::with_message(Errno::EOVERFLOW, "start overflow"))?,

            RangeLockWhence::SEEK_END => (file.path().inode().metadata().size as off_t)
                .checked_add(lock.l_start)
                .ok_or(Error::with_message(Errno::EOVERFLOW, "start overflow"))?,
        }
    };

    if start < 0 {
        return Err(Error::with_message(Errno::EINVAL, "invalid start"));
    }

    let (start, end) = match lock.l_len {
        len if len > 0 => {
            let end = start
                .checked_add(len)
                .ok_or(Error::with_message(Errno::EOVERFLOW, "end overflow"))?;
            (start as usize, end as usize)
        }
        0 => (start as usize, OFFSET_MAX),
        len if len < 0 => {
            let end = start;
            // `start + len` won't overflow because `start >= 0` and `len < 0`.
            let new_start = start + len;
            if new_start < 0 {
                return Err(Error::with_message(Errno::EINVAL, "invalid len"));
            }
            (new_start as usize, end as usize)
        }
        _ => unreachable!(),
    };

    FileRange::new(start, end)
}

fn lease_type_from_arg(arg: u64) -> Result<RangeLockType> {
    let lease_type =
        u16::try_from(arg).map_err(|_| Error::with_message(Errno::EINVAL, "invalid lease type"))?;
    Ok(RangeLockType::try_from(lease_type)?)
}

fn file_async_owner_from_setown_arg(arg: u64) -> Result<Option<FileAsyncOwner>> {
    let owner_id = arg as i32;
    if owner_id == 0 {
        return Ok(None);
    }

    if owner_id > 0 {
        let pid = u32::try_from(owner_id)
            .map_err(|_| Error::with_message(Errno::EINVAL, "invalid process id"))?;
        let pid = Pid::try_from(pid)
            .map_err(|_| Error::with_message(Errno::EINVAL, "invalid process id"))?;
        if process_table::get_process(pid).is_none() {
            return_errno_with_message!(Errno::ESRCH, "cannot set owner with an invalid pid");
        }
        return Ok(Some(FileAsyncOwner::Process(pid)));
    }

    let pgid = owner_id.unsigned_abs();
    let pgid = Pgid::try_from(pgid)
        .map_err(|_| Error::with_message(Errno::EINVAL, "invalid process group id"))?;
    if process_table::get_process_group(&pgid).is_none() {
        return_errno_with_message!(Errno::ESRCH, "cannot set owner with an invalid pgid");
    }
    Ok(Some(FileAsyncOwner::ProcessGroup(pgid)))
}

fn signal_from_setsig_arg(arg: u64) -> Result<Option<SigNum>> {
    if arg == 0 {
        return Ok(None);
    }

    let signal = u8::try_from(arg)
        .map_err(|_| Error::with_message(Errno::EINVAL, "invalid signal number"))?;
    let signal = SigNum::try_from(signal)?;
    if signal == SIGKILL || signal == SIGSTOP {
        return_errno_with_message!(
            Errno::EINVAL,
            "SIGKILL and SIGSTOP are not allowed for F_SETSIG"
        );
    }
    if signal == SIGIO {
        return Ok(None);
    }

    Ok(Some(signal))
}
