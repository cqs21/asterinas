// SPDX-License-Identifier: MPL-2.0

use alloc::collections::btree_set::BTreeSet;
use core::sync::atomic::{AtomicU32, Ordering};

use ostd::{sync::RwLock, Error};
use spin::Once;

/// The upper limit for block device major numbers (not included).
const MAJOR_MAX: u32 = 512;

const LAST_DYNAMIC_MAJOR: u32 = 254;

/// The upper limit for block device minor numbers (not included).
const MINOR_MAX: u32 = 1 << 20;

static MAJORS: RwLock<BTreeSet<u32>> = RwLock::new(BTreeSet::new());

/// Registers a block device major number.
///
/// This function registers a major device number for block devices. If the
/// requested major number is 0, a dynamic major number will be allocated.
pub fn register_device_ids(major: u32) -> Result<DeviceIdAllocator, Error> {
    if major >= MAJOR_MAX {
        return Err(Error::InvalidArgs);
    }

    let mut majors = MAJORS.write();
    if major == 0 {
        for id in (1..LAST_DYNAMIC_MAJOR + 1).rev() {
            if majors.insert(id) {
                return Ok(DeviceIdAllocator::new(id));
            }
        }
        return Err(Error::NotEnoughResources);
    }

    if majors.insert(major) {
        return Ok(DeviceIdAllocator::new(major));
    }

    Err(Error::NotEnoughResources)
}

/// An allocator for block device IDs.
#[derive(Debug)]
pub struct DeviceIdAllocator {
    major: u32,
    allocated_minors: RwLock<BTreeSet<u32>>,
}

impl DeviceIdAllocator {
    fn new(major: u32) -> Self {
        Self {
            major,
            allocated_minors: RwLock::new(BTreeSet::new()),
        }
    }

    /// Returns the major device number.
    pub fn major(&self) -> u32 {
        self.major
    }

    /// Allocates a specific minor device number.
    ///
    /// This method attempts to allocate a specific minor device number within the
    /// range managed by this allocator. If the minor number is outside the managed
    /// range or has already been allocated, the method returns `None`.
    pub fn allocate(&self, minor: u32) -> Option<(u32, u32)> {
        if minor >= MINOR_MAX {
            return None;
        }

        if !self.allocated_minors.write().insert(minor) {
            return None;
        }

        Some((self.major, minor))
    }

    /// Releases a previously allocated minor device number.
    ///
    /// Returns `true` if the minor number was successfully released, or `false`
    /// if it was not previously allocated.
    pub fn release(&self, minor: u32) -> bool {
        self.allocated_minors.write().remove(&minor)
    }
}

impl Drop for DeviceIdAllocator {
    fn drop(&mut self) {
        MAJORS.write().remove(&self.major);
    }
}

/// The major device number used for extended partitions when the number
/// of disk partitions exceeds the standard limit.
const EXTENDED_MAJOR: u32 = 259;

/// The next available minor device number for extended partitions.
static NEXT_EXTENDED_MINOR: AtomicU32 = AtomicU32::new(0);

static EXTENDED_DEVICE_ID_ALLOCATOR: Once<DeviceIdAllocator> = Once::new();

pub(super) fn init() {
    EXTENDED_DEVICE_ID_ALLOCATOR.call_once(|| register_device_ids(EXTENDED_MAJOR).unwrap());
}

/// Allocates an extended device ID.
///
/// If the request minor number is not available, the next available minor number will be used.
pub fn allocate_extended_device_id(minor: u32) -> (u32, u32) {
    let ida = EXTENDED_DEVICE_ID_ALLOCATOR.get().unwrap();

    if let Some(id) = ida.allocate(minor) {
        return id;
    };

    let minor = NEXT_EXTENDED_MINOR.fetch_add(1, Ordering::Relaxed);
    ida.allocate(minor).unwrap()
}

/// Releases an extended device ID.
pub fn release_extended_device_id(minor: u32) {
    EXTENDED_DEVICE_ID_ALLOCATOR.get().unwrap().release(minor);
}
