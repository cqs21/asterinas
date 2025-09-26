// SPDX-License-Identifier: MPL-2.0

use alloc::collections::btree_set::BTreeSet;
use core::{fmt::Debug, ops::Range};

use ostd::sync::RwLock;

pub const MAJOR_BITS: usize = 12;
pub const MINOR_BITS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Block,
    Char,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId {
    major: u32,
    minor: u32,
}

impl DeviceId {
    fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    pub fn major(&self) -> u32 {
        self.major
    }

    pub fn minor(&self) -> u32 {
        self.minor
    }
}

impl DeviceId {
    /// Creates a device ID from the encoded `u64` value.
    ///
    /// See [`as_encoded_u64`] for details about how to encode a device ID to a `u64` value.
    ///
    /// [`as_encoded_u64`]: Self::as_encoded_u64
    pub fn from_encoded_u64(raw: u64) -> Self {
        let major = ((raw >> 32) & 0xffff_f000 | (raw >> 8) & 0x0000_0fff) as u32;
        let minor = ((raw >> 12) & 0xffff_ff00 | raw & 0x0000_00ff) as u32;
        Self::new(major, minor)
    }

    /// Encodes the device ID as a `u64` value.
    ///
    /// The lower 32 bits use the same encoding strategy as Linux. See the Linux implementation at:
    /// <https://github.com/torvalds/linux/blob/0ff41df1cb268fc69e703a08a57ee14ae967d0ca/include/linux/kdev_t.h#L39-L44>.
    ///
    /// If the major or minor device number is too large, the additional bits will be recorded
    /// using the higher 32 bits. Note that as of 2025, the Linux kernel still has no support for
    /// 64-bit device IDs:
    /// <https://github.com/torvalds/linux/blob/0ff41df1cb268fc69e703a08a57ee14ae967d0ca/include/linux/types.h#L18>.
    /// So this encoding follows the implementation in glibc:
    /// <https://github.com/bminor/glibc/blob/632d895f3e5d98162f77b9c3c1da4ec19968b671/bits/sysmacros.h#L26-L34>.
    pub fn as_encoded_u64(&self) -> u64 {
        let major = self.major() as u64;
        let minor = self.minor() as u64;
        ((major & 0xffff_f000) << 32)
            | ((major & 0x0000_0fff) << 8)
            | ((minor & 0xffff_ff00) << 12)
            | (minor & 0x0000_00ff)
    }
}

#[derive(Debug)]
pub struct DeviceIdAllocator {
    pub type_: DeviceType,
    pub major: u32,
    pub minors: Range<u32>,
    used: RwLock<BTreeSet<u32>>,
}

impl DeviceIdAllocator {
    fn new(type_: DeviceType, major: u32, minors: Range<u32>) -> Self {
        Self {
            type_,
            major,
            minors,
            used: RwLock::new(BTreeSet::new()),
        }
    }

    pub fn allocate(&self, minor: u32) -> Option<DeviceId> {
        if !self.minors.contains(&minor) {
            return None;
        }

        if !self.used.write().insert(minor) {
            return None;
        }

        Some(DeviceId {
            major: self.major,
            minor,
        })
    }

    pub fn release(&self, minor: u32) -> bool {
        self.used.write().remove(&minor)
    }
}

impl Drop for DeviceIdAllocator {
    fn drop(&mut self) {
        match self.type_ {
            DeviceType::Block => super::block::unregister_device_ids(self.major),
            DeviceType::Char => super::char::unregister_device_ids(self.major, &self.minors),
            DeviceType::Other => (),
        }
    }
}

pub fn register_device_ids(
    type_: DeviceType,
    major: u32,
    minors: Range<u32>,
) -> Result<DeviceIdAllocator, ostd::Error> {
    let major = match type_ {
        DeviceType::Block => super::block::register_device_ids(major)?,
        DeviceType::Char => super::char::register_device_ids(major, &minors)?,
        DeviceType::Other => return Err(ostd::Error::InvalidArgs),
    };

    Ok(DeviceIdAllocator::new(type_, major, minors))
}
