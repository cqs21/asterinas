// SPDX-License-Identifier: MPL-2.0

use alloc::{boxed::ThinBox, collections::BTreeSet};

use crate::{
    fs::{
        file::{AccessMode, flock::FlockList},
        vfs::{
            inode::Inode,
            notify::FsEventPublisher,
            range_lock::{RangeLockList, RangeLockType},
        },
    },
    prelude::*,
    process::{
        Pid, process_table,
        signal::{constants::SIGIO, signals::kernel::KernelSignal},
    },
};

#[derive(Debug, Clone, Copy)]
struct FileLease {
    owner_id: u64,
    owner_pid: Pid,
    type_: RangeLockType,
    has_pending_write_break: bool,
}

pub type LeaseType = RangeLockType;

/// Context for FS locks.
pub struct FsLockContext {
    range_lock_list: RangeLockList,
    flock_list: FlockList,
    open_file_description_ids: Mutex<BTreeSet<u64>>,
    lease: Mutex<Option<FileLease>>,
}

impl FsLockContext {
    pub(self) fn new() -> Self {
        Self {
            range_lock_list: RangeLockList::new(),
            flock_list: FlockList::new(),
            open_file_description_ids: Mutex::new(BTreeSet::new()),
            lease: Mutex::new(None),
        }
    }

    /// Returns a reference to the range lock list.
    pub fn range_lock_list(&self) -> &RangeLockList {
        &self.range_lock_list
    }

    /// Returns a reference to the flock list.
    pub fn flock_list(&self) -> &FlockList {
        &self.flock_list
    }

    pub fn get_lease(&self) -> LeaseType {
        self.lease
            .lock()
            .as_ref()
            .map(|lease| lease.type_)
            .unwrap_or(LeaseType::Unlock)
    }

    pub fn register_open_file_description(&self, owner_id: u64) {
        self.open_file_description_ids.lock().insert(owner_id);
    }

    pub fn release_open_file_description(&self, owner_id: u64) {
        self.open_file_description_ids.lock().remove(&owner_id);
    }

    pub fn set_lease(&self, owner_id: u64, owner_pid: Pid, lease_type: LeaseType) -> Result<()> {
        debug_assert_ne!(lease_type, LeaseType::Unlock);

        if self
            .open_file_description_ids
            .lock()
            .iter()
            .any(|open_id| *open_id != owner_id)
        {
            return_errno_with_message!(
                Errno::EBUSY,
                "cannot set a lease while another file description is open"
            );
        }

        let mut lease = self.lease.lock();
        if let Some(existing_lease) = lease.as_ref()
            && existing_lease.owner_id != owner_id
        {
            return_errno_with_message!(Errno::EBUSY, "the file already has a lease");
        }
        if let Some(existing_lease) = lease.as_ref()
            && existing_lease.owner_id == owner_id
            && existing_lease.type_ == LeaseType::WriteLock
            && lease_type == LeaseType::ReadLock
            && existing_lease.has_pending_write_break
        {
            return_errno_with_message!(
                Errno::EAGAIN,
                "cannot downgrade a write lease while a write break is pending"
            );
        }

        *lease = Some(FileLease {
            owner_id,
            owner_pid,
            type_: lease_type,
            has_pending_write_break: false,
        });
        Ok(())
    }

    pub fn notify_lease_break_for_open(&self, access_mode: AccessMode, requester_pid: Option<Pid>) {
        let mut lease_guard = self.lease.lock();
        let Some(lease) = lease_guard.as_mut() else {
            return;
        };

        let has_conflict = match lease.type_ {
            LeaseType::ReadLock => access_mode.is_writable(),
            LeaseType::WriteLock => true,
            LeaseType::Unlock => false,
        };
        if !has_conflict {
            return;
        }

        if lease.type_ == LeaseType::WriteLock {
            lease.has_pending_write_break = access_mode.is_writable();
        }

        let lease = *lease;
        drop(lease_guard);
        self.enqueue_lease_break_signal(lease, requester_pid);
    }

    pub fn notify_lease_break_for_truncate(&self, requester_pid: Option<Pid>) {
        let mut lease_guard = self.lease.lock();
        let Some(lease) = lease_guard.as_mut() else {
            return;
        };

        if lease.type_ == LeaseType::WriteLock {
            lease.has_pending_write_break = true;
        }
        let lease = *lease;
        drop(lease_guard);
        self.enqueue_lease_break_signal(lease, requester_pid);
    }

    fn enqueue_lease_break_signal(&self, lease: FileLease, requester_pid: Option<Pid>) {
        if requester_pid.is_some_and(|pid| pid == lease.owner_pid) {
            return;
        }

        if let Some(lease_holder) = process_table::get_process(lease.owner_pid) {
            lease_holder.enqueue_signal(Box::new(KernelSignal::new(SIGIO)));
        }
    }

    pub fn release_lease(&self, owner_id: u64) {
        let mut lease = self.lease.lock();
        if lease
            .as_ref()
            .is_some_and(|existing_lease| existing_lease.owner_id == owner_id)
        {
            *lease = None;
        }
    }
}

/// A trait that instantiates kernel types for the inode [`Extension`].
///
/// [`Extension`]: super::inode::Extension
pub trait InodeExt {
    /// Gets or initializes the FS event publisher.
    ///
    /// If the publisher does not exist for this inode, it will be created.
    fn fs_event_publisher_or_init(&self) -> &FsEventPublisher;

    /// Returns a reference to the FS event publisher.
    ///
    /// If the publisher does not exist for this inode, a [`None`] will be returned.
    fn fs_event_publisher(&self) -> Option<&FsEventPublisher>;

    /// Gets or initializes the FS lock context.
    ///
    /// If the context does not exist for this inode, it will be created.
    fn fs_lock_context_or_init(&self) -> &FsLockContext;

    /// Returns a reference to the FS lock context.
    ///
    /// If the context does not exist for this inode, a [`None`] will be returned.
    fn fs_lock_context(&self) -> Option<&FsLockContext>;
}

impl InodeExt for dyn Inode {
    fn fs_event_publisher_or_init(&self) -> &FsEventPublisher {
        self.extension()
            .group1()
            .call_once(|| ThinBox::new_unsize(FsEventPublisher::new()))
            .downcast_ref()
            .unwrap()
    }

    fn fs_event_publisher(&self) -> Option<&FsEventPublisher> {
        Some(self.extension().group1().get()?.downcast_ref().unwrap())
    }

    fn fs_lock_context_or_init(&self) -> &FsLockContext {
        self.extension()
            .group2()
            .call_once(|| ThinBox::new_unsize(FsLockContext::new()))
            .downcast_ref()
            .unwrap()
    }

    fn fs_lock_context(&self) -> Option<&FsLockContext> {
        Some(self.extension().group2().get()?.downcast_ref().unwrap())
    }
}
