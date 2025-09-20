// SPDX-License-Identifier: MPL-2.0

use alloc::{collections::btree_set::BTreeSet, sync::Arc};
use core::{fmt::Debug, ops::Range};

use ostd::sync::RwLock;

pub const MAJOR_BITS: usize = 12;
pub const MINOR_BITS: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum DeviceType {
    Block,
    Character,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId {
    pub major: u32,
    pub minor: u32,
}

pub trait MinorIdAllocator: Send + Sync + Debug {
    fn allocate(&self, minor: u32) -> Option<u32>;
    fn release(&self, minor: u32) -> bool;
}

#[derive(Debug)]
pub struct NoConflictMinorIdAllocator {
    used: RwLock<BTreeSet<u32>>,
}

impl NoConflictMinorIdAllocator {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            used: RwLock::new(BTreeSet::new()),
        })
    }
}

impl MinorIdAllocator for NoConflictMinorIdAllocator {
    fn allocate(&self, minor: u32) -> Option<u32> {
        if self.used.write().insert(minor) {
            Some(minor)
        } else {
            None
        }
    }

    fn release(&self, minor: u32) -> bool {
        self.used.write().remove(&minor)
    }
}

#[derive(Debug)]
pub struct DeviceIdAllocator {
    pub type_: DeviceType,
    pub major: u32,
    pub minors: Range<u32>,
    minor_allocator: Arc<dyn MinorIdAllocator>,
}

impl DeviceIdAllocator {
    fn new(
        type_: DeviceType,
        major: u32,
        minors: Range<u32>,
        minor_allocator: Arc<dyn MinorIdAllocator>,
    ) -> Self {
        Self {
            type_,
            major,
            minors,
            minor_allocator,
        }
    }

    pub fn allocate(&self, minor: u32) -> Option<DeviceId> {
        let minor = self.minor_allocator.allocate(minor)?;
        if !self.minors.contains(&minor) {
            return None;
        }
        Some(DeviceId {
            major: self.major,
            minor,
        })
    }

    pub fn release(&self, minor: u32) -> bool {
        self.minor_allocator.release(minor)
    }
}

pub fn register_device_ids(
    type_: DeviceType,
    major: u32,
    minors: Range<u32>,
    minor_allocator: Arc<dyn MinorIdAllocator>,
) -> Result<DeviceIdAllocator, ostd::Error> {
    let major = match type_ {
        DeviceType::Block => super::block::register_device_ids(major)?,
        DeviceType::Character => super::char::register_device_ids(major, &minors)?,
        DeviceType::Unknown => return Err(ostd::Error::InvalidArgs),
    };

    Ok(DeviceIdAllocator::new(
        type_,
        major,
        minors,
        minor_allocator,
    ))
}

pub fn unregister_device_ids(ida: &Arc<DeviceIdAllocator>) -> Result<(), ostd::Error> {
    match ida.type_ {
        DeviceType::Block => super::block::unregister_device_ids(ida.major),
        DeviceType::Character => super::char::unregister_device_ids(ida.major, &ida.minors),
        DeviceType::Unknown => return Err(ostd::Error::InvalidArgs),
    }
}
