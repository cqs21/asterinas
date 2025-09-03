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
#![feature(fn_traits)]
#![feature(step_trait)]
#![feature(trait_upcasting)]

extern crate alloc;

pub mod bio;
pub mod id;
mod impl_block_device;
mod partition;
mod prelude;
pub mod request_queue;
pub mod sysnode;

use component::{init_component, ComponentInitError};
use spin::Once;
use sysnode::{BlockSysNode, DeviceManager};

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

    /// Returns a `SysTree` node that represents the block device under `/sys/block`.
    fn sysnode(&self) -> Arc<BlockSysNode>;
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
    DEVICE_MANAGER.get().unwrap().register_device(device)
}

pub fn get_device(name: &str) -> Option<Arc<dyn BlockDevice>> {
    DEVICE_MANAGER.get().unwrap().get_device(name)
}

pub fn all_devices() -> Vec<Arc<dyn BlockDevice>> {
    DEVICE_MANAGER.get().unwrap().all_devices()
}

static DEVICE_MANAGER: Once<Arc<DeviceManager>> = Once::new();

#[init_component]
fn init_early() -> Result<(), ComponentInitError> {
    DEVICE_MANAGER.call_once(DeviceManager::new);
    Ok(())
}

#[init_component("in_first_kthread")]
fn init_in_first_kthread() -> Result<(), ComponentInitError> {
    all_devices().iter().for_each(partition::parse);
    Ok(())
}
