// SPDX-License-Identifier: MPL-2.0

//! The block devices of Asterinas.
//！
//！This crate provides a number of base components for block devices, including
//! an abstraction of block devices, as well as the registration and lookup of block devices.
//!
//! Block devices use a queue-based model for asynchronous I/O operations. It is necessary
//! for a block device to maintain a queue to handle I/O requests. The users (e.g., fs)
//! submit I/O requests to this queue and wait for their completion. Drivers implementing
//! block devices can create their own queues as needed, with the possibility to reorder
//! and merge requests within the queue.
//!
//! This crate also offers the `Bio` related data structures and APIs to accomplish
//! safe and convenient block I/O operations, for example:
//!
//! ```no_run
//! // Creates a bio request.
//! let bio = Bio::new(BioType::Write, sid, segments, None);
//! // Submits to the block device.
//! let bio_waiter = bio.submit(block_device)?;
//! // Waits for the the completion.
//! let Some(status) = bio_waiter.wait() else {
//!     return Err(IoError);
//! };
//! assert!(status == BioStatus::Complete);
//! ```
//!
#![no_std]
#![deny(unsafe_code)]
#![feature(step_trait)]
#![feature(trait_upcasting)]
#![expect(dead_code)]

extern crate alloc;

pub mod bio;
pub mod id;
mod impl_block_device;
mod partition;
mod prelude;
pub mod request_queue;
mod sysnode;

use aster_device::{
    register_device_ids, Device, DeviceId, DeviceIdAllocator, DeviceType, MINOR_BITS,
};
use component::{init_component, ComponentInitError};
use spin::Once;
use sysnode::DeviceManager;

use self::{
    bio::{BioEnqueueError, SubmittedBio},
    prelude::*,
};
pub use crate::partition::{PartitionInfo, PartitionNode};

pub const BLOCK_SIZE: usize = ostd::mm::PAGE_SIZE;
pub const SECTOR_SIZE: usize = 512;

pub trait BlockDevice: Device + Any {
    /// Enqueues a new `SubmittedBio` to the block device.
    fn enqueue(&self, bio: SubmittedBio) -> Result<(), BioEnqueueError>;

    /// Returns the metadata of the block device.
    fn metadata(&self) -> BlockDeviceMeta;

    /// Returns the device ID allocator of the block device.
    fn id_allocator(&self) -> &'static DeviceIdAllocator;

    /// Returns whether the block device is a partition.
    fn is_partition(&self) -> bool {
        false
    }

    /// Sets the partitions of the block device.
    fn set_partitions(&self, _infos: Vec<Option<PartitionInfo>>) {}

    /// Returns the partitions of the block device.
    fn partitions(&self) -> Option<Vec<Arc<dyn BlockDevice>>> {
        None
    }
}

/// Metadata for a block device.
#[derive(Debug, Default, Clone, Copy)]
pub struct BlockDeviceMeta {
    /// The upper limit for the number of segments per bio.
    pub max_nr_segments_per_bio: usize,
    /// The total number of sectors of the block device.
    pub nr_sectors: usize,
    // Additional useful metadata can be added here in the future.
}

impl dyn BlockDevice {
    pub fn downcast_ref<T: BlockDevice>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

pub fn register_device(device: Arc<dyn BlockDevice>) {
    DEVICE_MANAGER.get().unwrap().register_device(device);
}

pub fn get_device(name: &str) -> Option<Arc<dyn BlockDevice>> {
    DEVICE_MANAGER.get().unwrap().get_device(name)
}

pub fn all_devices() -> Vec<Arc<dyn BlockDevice>> {
    DEVICE_MANAGER.get().unwrap().all_devices()
}

/// If a disk has more than 16 partitions, the extended major:minor numbers will be assigned.
pub const LEGACY_PARTITION_LIMIT: u32 = 16;

/// The major device number used for extended partitions when the number
/// of disk partitions exceeds the standard limit.
const EXTENDED_MAJOR: u32 = 259;

/// The upper limit for extended block device minor numbers (not included).
const EXTENDED_MINOR_MAX: u32 = 1 << MINOR_BITS;

/// The next available minor device number for extended partitions.
static NEXT_EXTENDED_MINOR: AtomicU32 = AtomicU32::new(0);

static EXTENDED_BLOCK_ID_ALLOCATOR: Once<DeviceIdAllocator> = Once::new();

/// Allocates an extended device ID.
///
/// If the request minor number is not available, the next available minor number will be used.
pub fn alloc_extended_device_id(minor: u32) -> DeviceId {
    let ida = EXTENDED_BLOCK_ID_ALLOCATOR.get().unwrap();

    if let Some(id) = ida.allocate(minor) {
        return id;
    };

    let minor = NEXT_EXTENDED_MINOR.fetch_add(1, Ordering::Relaxed);
    ida.allocate(minor).unwrap()
}

/// Releases an extended device ID.
pub fn release_extended_device_id(id: DeviceId) {
    if id.major() != EXTENDED_MAJOR {
        return;
    }

    EXTENDED_BLOCK_ID_ALLOCATOR
        .get()
        .unwrap()
        .release(id.minor());
}

static DEVICE_MANAGER: Once<Arc<DeviceManager>> = Once::new();

#[init_component]
fn init() -> Result<(), ComponentInitError> {
    DEVICE_MANAGER.call_once(DeviceManager::new);
    EXTENDED_BLOCK_ID_ALLOCATOR.call_once(|| {
        register_device_ids(DeviceType::Block, EXTENDED_MAJOR, 0..EXTENDED_MINOR_MAX).unwrap()
    });

    Ok(())
}

#[init_component(process)]
fn init_in_first_process() -> Result<(), component::ComponentInitError> {
    for device in all_devices() {
        let partitions = partition::parse(&device);
        device.set_partitions(partitions);
    }

    Ok(())
}
