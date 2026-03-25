// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::{AtomicU8, Ordering};

use aster_util::slot_vec::SlotVec;

use super::{StatusFlags, file_handle::FileLike};
use crate::{
    events::{IoEvents, Observer},
    prelude::*,
    process::{
        Pgid, Pid,
        posix_thread::FileTableRefMut,
        process_table,
        signal::{PollAdaptor, constants::SIGIO, sig_num::SigNum, signals::kernel::KernelSignal},
    },
    thread::Tid,
};

pub type FileDesc = i32;

#[derive(Clone)]
pub struct FileTable {
    table: SlotVec<FileTableEntry>,
}

impl FileTable {
    pub const fn new() -> Self {
        Self {
            table: SlotVec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.table.slots_len()
    }

    fn min_free_fd_from(&self, min_fd: usize, max_fd_exclusive: usize) -> Option<usize> {
        if min_fd >= max_fd_exclusive {
            return None;
        }

        let slots_len = self.table.slots_len();
        if min_fd >= slots_len {
            return Some(min_fd);
        }

        for fd in min_fd..slots_len.min(max_fd_exclusive) {
            if self.table.get(fd).is_none() {
                return Some(fd);
            }
        }

        (slots_len < max_fd_exclusive).then_some(slots_len)
    }

    /// Duplicates `fd` onto the lowest-numbered available descriptor equal to
    /// or greater than `ceil_fd`.
    pub fn dup_ceil(
        &mut self,
        fd: FileDesc,
        ceil_fd: FileDesc,
        flags: FdFlags,
    ) -> Result<FileDesc> {
        self.dup_ceil_with_limit(fd, ceil_fd, usize::MAX, flags)
    }

    /// Duplicates `fd` onto the lowest-numbered available descriptor equal to
    /// or greater than `ceil_fd` and strictly less than `max_fd_exclusive`.
    pub fn dup_ceil_with_limit(
        &mut self,
        fd: FileDesc,
        ceil_fd: FileDesc,
        max_fd_exclusive: usize,
        flags: FdFlags,
    ) -> Result<FileDesc> {
        if ceil_fd < 0 {
            return_errno_with_message!(Errno::EINVAL, "the minimum fd must be non-negative");
        }

        let entry = self.duplicate_entry(fd, flags)?;

        // Get the lowest-numbered available fd equal to or greater than `ceil_fd`.
        let min_free_fd = self
            .min_free_fd_from(ceil_fd as usize, max_fd_exclusive)
            .ok_or_else(|| {
                Error::with_message(
                    Errno::EMFILE,
                    "no file descriptor available under the limit",
                )
            })?;
        self.table.put_at(min_free_fd, entry);
        Ok(min_free_fd as FileDesc)
    }

    /// Duplicates `fd` onto the exact descriptor number `new_fd`.
    pub fn dup_exact(
        &mut self,
        fd: FileDesc,
        new_fd: FileDesc,
        flags: FdFlags,
    ) -> Result<Option<Arc<dyn FileLike>>> {
        let entry = self.duplicate_entry(fd, flags)?;
        let closed_file = self.close_file(new_fd);
        self.table.put_at(new_fd as usize, entry);
        Ok(closed_file)
    }

    fn duplicate_entry(&self, fd: FileDesc, flags: FdFlags) -> Result<FileTableEntry> {
        let file = self
            .table
            .get(fd as usize)
            .map(|entry| entry.file.clone())
            .ok_or(Error::with_message(Errno::EBADF, "fd does not exist"))?;
        Ok(FileTableEntry::new(file, flags))
    }

    pub fn insert(&mut self, item: Arc<dyn FileLike>, flags: FdFlags) -> FileDesc {
        let entry = FileTableEntry::new(item, flags);
        self.table.put(entry) as FileDesc
    }

    pub fn insert_with_limit(
        &mut self,
        item: Arc<dyn FileLike>,
        flags: FdFlags,
        max_fd_exclusive: usize,
    ) -> Result<FileDesc> {
        let entry = FileTableEntry::new(item, flags);

        let min_free_fd = self.min_free_fd_from(0, max_fd_exclusive).ok_or_else(|| {
            Error::with_message(
                Errno::EMFILE,
                "no file descriptor available under the limit",
            )
        })?;

        self.table.put_at(min_free_fd, entry);
        Ok(min_free_fd as FileDesc)
    }

    pub fn close_file(&mut self, fd: FileDesc) -> Option<Arc<dyn FileLike>> {
        let removed_entry = self.table.remove(fd as usize)?;
        // POSIX record locks are process-associated and Linux drops them when any fd for the inode is
        // closed by that process, even if duplicated descriptors still exist.
        //
        // Reference: <https://man7.org/linux/man-pages/man2/fcntl_locking.2.html>
        if let Ok(inode_handle) = removed_entry.file.as_inode_handle_or_err() {
            inode_handle.release_range_locks();
        }
        Some(removed_entry.file)
    }

    pub fn close_files_on_exec(&mut self) -> Vec<Arc<dyn FileLike>> {
        self.close_files(|entry| entry.flags().contains(FdFlags::CLOEXEC))
    }

    fn close_files<F>(&mut self, should_close: F) -> Vec<Arc<dyn FileLike>>
    where
        F: Fn(&FileTableEntry) -> bool,
    {
        let mut closed_files = Vec::new();
        let closed_fds: Vec<FileDesc> = self
            .table
            .idxes_and_items()
            .filter_map(|(idx, entry)| {
                if should_close(entry) {
                    Some(idx as FileDesc)
                } else {
                    None
                }
            })
            .collect();

        for fd in closed_fds {
            closed_files.push(self.close_file(fd).unwrap());
        }

        closed_files
    }

    pub fn get_file(&self, fd: FileDesc) -> Result<&Arc<dyn FileLike>> {
        self.table
            .get(fd as usize)
            .map(|entry| entry.file())
            .ok_or(Error::with_message(Errno::EBADF, "fd not exits"))
    }

    pub fn get_entry(&self, fd: FileDesc) -> Result<&FileTableEntry> {
        self.table
            .get(fd as usize)
            .ok_or(Error::with_message(Errno::EBADF, "fd not exits"))
    }

    pub fn get_entry_mut(&mut self, fd: FileDesc) -> Result<&mut FileTableEntry> {
        self.table
            .get_mut(fd as usize)
            .ok_or(Error::with_message(Errno::EBADF, "fd not exits"))
    }

    pub fn fds_and_files(&self) -> impl Iterator<Item = (FileDesc, &'_ Arc<dyn FileLike>)> {
        self.table
            .idxes_and_items()
            .map(|(idx, entry)| (idx as FileDesc, entry.file()))
    }
}

impl Default for FileTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FileTable {
    fn drop(&mut self) {
        // Exit paths may drop the whole file table directly instead of closing file
        // descriptors one by one. Reuse the normal close path so process-associated
        // record locks are released before the table disappears.
        self.close_files(|_| true);
    }
}

/// A helper trait that provides methods to operate the file table.
pub trait WithFileTable {
    /// Calls `f` with the file table.
    ///
    /// This method is lockless if the file table is not shared. Otherwise, `f` is called while
    /// holding the read lock on the file table.
    fn read_with<R>(&mut self, f: impl FnOnce(&FileTable) -> R) -> R;
}

impl WithFileTable for FileTableRefMut<'_> {
    fn read_with<R>(&mut self, f: impl FnOnce(&FileTable) -> R) -> R {
        let file_table = self.unwrap();

        if let Some(inner) = file_table.get() {
            f(inner)
        } else {
            f(&file_table.read())
        }
    }
}

/// Gets a file from a file descriptor as fast as possible.
///
/// `file_table` should be a mutable borrow of the file table contained in the `file_table` field
/// (which is a [`RefCell`]) in [`ThreadLocal`]. A mutable borrow is required because its
/// exclusivity can be useful for achieving lockless file lookups.
///
/// If the file table is not shared with another thread, this macro will be free of locks
/// ([`RwArc::read`]) and free of reference counting ([`Arc::clone`]).
///
/// If the file table is shared, the read lock is taken, the file is cloned, and then the read lock
/// is released. Cloning and releasing the lock is necessary because we cannot hold such locks when
/// operating on files, since many operations on files can block.
///
/// Note: This has to be a macro due to a limitation in the Rust borrow check implementation. Once
/// <https://github.com/rust-lang/rust/issues/58910> is fixed, we can try to convert this macro to
/// a function.
///
/// [`RefCell`]: core::cell::RefCell
/// [`ThreadLocal`]: crate::process::posix_thread::ThreadLocal
/// [`RwArc::read`]: ostd::sync::RwArc::read
macro_rules! get_file_fast {
    ($file_table:expr, $file_desc:expr) => {{
        use alloc::borrow::Cow;

        use ostd::sync::RwArc;
        use $crate::{
            fs::file::file_table::{FileDesc, FileTable},
            process::posix_thread::FileTableRefMut,
        };

        let file_table: &mut FileTableRefMut<'_> = $file_table;
        let file_table: &mut RwArc<FileTable> = file_table.unwrap();
        let file_desc: FileDesc = $file_desc;

        if let Some(inner) = file_table.get() {
            // Fast path: The file table is not shared, we can get the file in a lockless way.
            Cow::Borrowed(inner.get_file(file_desc)?)
        } else {
            // Slow path: The file table is shared, we need to hold the lock and clone the file.
            Cow::Owned(file_table.read().get_file(file_desc)?.clone())
        }
    }};
}

pub(crate) use get_file_fast;

pub struct FileTableEntry {
    file: Arc<dyn FileLike>,
    flags: AtomicU8,
    owner: Option<OwnerRegistration>,
    signal: Option<SigNum>,
}

impl FileTableEntry {
    pub fn new(file: Arc<dyn FileLike>, flags: FdFlags) -> Self {
        Self {
            file,
            flags: AtomicU8::new(flags.bits()),
            owner: None,
            signal: None,
        }
    }

    pub fn file(&self) -> &Arc<dyn FileLike> {
        &self.file
    }

    pub fn owner(&self) -> Option<FileAsyncOwner> {
        self.owner.as_ref().map(|registration| registration.target)
    }

    /// Set a process (group) as owner of the file descriptor.
    ///
    /// Such that this process (group) will receive `SIGIO` and `SIGURG` signals
    /// for I/O events on the file descriptor, if `O_ASYNC` status flag is set
    /// on this file.
    pub fn set_owner(&mut self, owner: Option<FileAsyncOwner>) -> Result<()> {
        let Some(target) = owner else {
            self.owner = None;
            return Ok(());
        };

        self.owner = Some(OwnerRegistration::new(
            self.file.clone(),
            target,
            self.signal,
        )?);

        Ok(())
    }

    pub fn signal(&self) -> Option<SigNum> {
        self.signal
    }

    pub fn set_signal(&mut self, signal: Option<SigNum>) -> Result<()> {
        self.signal = signal;

        if let Some(target) = self.owner.as_ref().map(|registration| registration.target) {
            self.owner = Some(OwnerRegistration::new(self.file.clone(), target, signal)?);
        }

        Ok(())
    }

    pub fn flags(&self) -> FdFlags {
        FdFlags::from_bits(self.flags.load(Ordering::Relaxed)).unwrap()
    }

    pub fn set_flags(&self, flags: FdFlags) {
        self.flags.store(flags.bits(), Ordering::Relaxed);
    }
}

impl Clone for FileTableEntry {
    fn clone(&self) -> Self {
        Self {
            file: self.file.clone(),
            flags: AtomicU8::new(self.flags.load(Ordering::Relaxed)),
            owner: None,
            signal: self.signal,
        }
    }
}

bitflags! {
    pub struct FdFlags: u8 {
        /// Close on exec
        const CLOEXEC = 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAsyncOwner {
    Thread(Tid),
    Process(Pid),
    ProcessGroup(Pgid),
}

struct OwnerRegistration {
    target: FileAsyncOwner,
    _poller: PollAdaptor<OwnerObserver>,
}

impl OwnerRegistration {
    fn new(
        file: Arc<dyn FileLike>,
        target: FileAsyncOwner,
        signal: Option<SigNum>,
    ) -> Result<Self> {
        let mut poller =
            PollAdaptor::with_observer(OwnerObserver::new(file.clone(), target, signal));
        file.poll(IoEvents::IN | IoEvents::OUT, Some(poller.as_handle_mut()));
        Ok(Self {
            target,
            _poller: poller,
        })
    }
}

struct OwnerObserver {
    file: Arc<dyn FileLike>,
    target: FileAsyncOwner,
    signal: Option<SigNum>,
}

impl OwnerObserver {
    pub fn new(file: Arc<dyn FileLike>, target: FileAsyncOwner, signal: Option<SigNum>) -> Self {
        Self {
            file,
            target,
            signal,
        }
    }
}

impl Observer<IoEvents> for OwnerObserver {
    fn on_events(&self, _events: &IoEvents) {
        if self.file.status_flags().contains(StatusFlags::O_ASYNC) {
            let signal = self.signal.unwrap_or(SIGIO);
            enqueue_signal_to_owner_async(self.target, signal);
        }
    }
}

fn enqueue_signal_to_owner_async(target: FileAsyncOwner, signal: SigNum) {
    use crate::{process::posix_thread::AsPosixThread, thread::work_queue};

    work_queue::submit_work_func(
        move || match target {
            FileAsyncOwner::Thread(tid) => {
                if let Some(thread) = crate::process::posix_thread::thread_table::get_thread(tid)
                    && let Some(posix_thread) = thread.as_posix_thread()
                {
                    posix_thread.enqueue_signal(Box::new(KernelSignal::new(signal)));
                }
            }
            FileAsyncOwner::Process(pid) => {
                if let Some(process) = process_table::get_process(pid) {
                    process.enqueue_signal(Box::new(KernelSignal::new(signal)));
                }
            }
            FileAsyncOwner::ProcessGroup(pgid) => {
                if let Some(process_group) = process_table::get_process_group(&pgid) {
                    process_group.broadcast_signal(KernelSignal::new(signal));
                }
            }
        },
        work_queue::WorkPriority::High,
    );
}
