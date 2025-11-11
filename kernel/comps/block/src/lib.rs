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

extern crate alloc;

pub mod bio;
mod device_id;
pub mod id;
mod impl_block_device;
mod partition;
mod prelude;
pub mod request_queue;

use component::{init_component, ComponentInitError};
pub use device_id::{
    allocate_extended_device_id, register_device_ids, release_extended_device_id, DeviceIdAllocator,
};
use ostd::sync::Mutex;
pub use partition::{PartitionInfo, PartitionNode};

use self::{
    bio::{BioEnqueueError, SubmittedBio},
    prelude::*,
};

pub const BLOCK_SIZE: usize = ostd::mm::PAGE_SIZE;
pub const SECTOR_SIZE: usize = 512;

pub trait BlockDevice: Send + Sync + Any + Debug {
    /// Enqueues a new `SubmittedBio` to the block device.
    fn enqueue(&self, bio: SubmittedBio) -> Result<(), BioEnqueueError>;

    /// Returns the metadata of the block device.
    fn metadata(&self) -> BlockDeviceMeta;

    /// Returns the name of the block device.
    fn name(&self) -> &str;

    /// Returns the device ID of the block device.
    fn id(&self) -> (u32, u32);

    /// Returns the device ID allocator of the block device.
    fn id_allocator(&self) -> Arc<DeviceIdAllocator>;

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

/// Adds a block device to the manager.
pub fn add_device(name: String, device: Arc<dyn BlockDevice>) {
    if device.is_partition() {
        return;
    }

    let _ = DEVICE_TABLE.lock().insert(name, device);
}

/// Removes the block device with the given name.
pub fn remove_device(name: &str) {
    let _ = DEVICE_TABLE.lock().remove(name);
}

/// Returns the block device with the given name, maybe a partition.
pub fn get_device(name: &str) -> Option<Arc<dyn BlockDevice>> {
    let devices = DEVICE_TABLE.lock();
    let (key, device) = devices.iter().find(|(key, _)| name.contains(*key))?;

    if key == name {
        return Some(device.clone());
    }

    let partitions = device.partitions()?;
    partitions.into_iter().find(|p| p.name() == name)
}

/// Returns all block devices, excluding partitions.
pub fn all_devices() -> Vec<Arc<dyn BlockDevice>> {
    DEVICE_TABLE.lock().values().cloned().collect()
}

static DEVICE_TABLE: Mutex<BTreeMap<String, Arc<dyn BlockDevice>>> = Mutex::new(BTreeMap::new());

#[init_component]
fn init() -> Result<(), ComponentInitError> {
    device_id::init();

    Ok(())
}

#[init_component(process)]
fn init_in_first_process() -> Result<(), component::ComponentInitError> {
    for device in DEVICE_TABLE.lock().values() {
        let partitions = partition::parse(device);
        device.set_partitions(partitions);
    }

    Ok(())
}
