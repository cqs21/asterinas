// SPDX-License-Identifier: MPL-2.0

use alloc::boxed::ThinBox;

use crate::{
    fs::{
        file::flock::FlockList,
        vfs::{
            inode::Inode,
            notify::FsEventPublisher,
            range_lock::{RangeLockList, RangeLockType},
        },
    },
    prelude::*,
};

struct FileLease {
    owner_id: u64,
    type_: RangeLockType,
}

pub type LeaseType = RangeLockType;

/// Context for FS locks.
pub struct FsLockContext {
    range_lock_list: RangeLockList,
    flock_list: FlockList,
    lease: Mutex<Option<FileLease>>,
}

impl FsLockContext {
    pub(self) fn new() -> Self {
        Self {
            range_lock_list: RangeLockList::new(),
            flock_list: FlockList::new(),
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

    pub fn set_lease(&self, owner_id: u64, lease_type: LeaseType) -> Result<()> {
        debug_assert_ne!(lease_type, LeaseType::Unlock);

        let mut lease = self.lease.lock();
        if let Some(existing_lease) = lease.as_ref()
            && existing_lease.owner_id != owner_id
        {
            return_errno_with_message!(Errno::EBUSY, "the file already has a lease");
        }

        *lease = Some(FileLease {
            owner_id,
            type_: lease_type,
        });
        Ok(())
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
