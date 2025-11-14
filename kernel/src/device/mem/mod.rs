// SPDX-License-Identifier: MPL-2.0

//! Memory devices.
//!
//! Character device with major number 1. The minor numbers are mapped as follows:
//! - 1 = /dev/mem      Physical memory access
//! - 2 = /dev/kmem     OBSOLETE - replaced by /proc/kcore
//! - 3 = /dev/null     Null device
//! - 4 = /dev/port     I/O port access
//! - 5 = /dev/zero     Null byte source
//! - 6 = /dev/core     OBSOLETE - replaced by /proc/kcore
//! - 7 = /dev/full     Returns ENOSPC on write
//! - 8 = /dev/random   Nondeterministic random number gen.
//! - 9 = /dev/urandom  Faster, less secure random number gen.
//! - 10 = /dev/aio     Asynchronous I/O notification interface
//! - 11 = /dev/kmsg    Writes to this come out as printk's, reads export the buffered printk records.
//! - 12 = /dev/oldmem  OBSOLETE - replaced by /proc/vmcore
//!
//! See <https://www.kernel.org/doc/Documentation/admin-guide/devices.txt>.

mod file;

use alloc::sync::{Arc, Weak};

use device_id::{DeviceId, MajorId, MinorId};
use file::MemFile;
pub use file::{getrandom, geturandom};
use ostd::mm::{VmReader, VmWriter};
use spin::Once;

use super::char::{acquire_major, register, CharDevice, MajorIdOwner};
use crate::{
    events::IoEvents,
    fs::{
        device::{Device, DeviceType},
        inode_handle::FileIo,
        utils::StatusFlags,
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
};

/// A memory device.
#[derive(Debug)]
pub struct MemDevice {
    id: DeviceId,
    file: MemFile,
    weak_self: Weak<Self>,
}

impl MemDevice {
    fn new(file: MemFile) -> Arc<Self> {
        let major = MEM_MAJOR.get().unwrap().get();
        let minor = MinorId::new(file.minor());

        Arc::new_cyclic(|weak_self| Self {
            id: DeviceId::new(major, minor),
            file,
            weak_self: weak_self.clone(),
        })
    }
}

impl FileIo for MemDevice {
    fn read(&self, writer: &mut VmWriter, status_flags: StatusFlags) -> Result<usize> {
        self.file.read(writer, status_flags)
    }

    fn write(&self, reader: &mut VmReader, status_flags: StatusFlags) -> Result<usize> {
        self.file.write(reader, status_flags)
    }
}

impl Pollable for MemDevice {
    fn poll(&self, mask: IoEvents, poller: Option<&mut PollHandle>) -> IoEvents {
        self.file.poll(mask, poller)
    }
}

impl Device for MemDevice {
    fn type_(&self) -> DeviceType {
        DeviceType::Char
    }

    fn id(&self) -> DeviceId {
        self.id
    }

    fn open(&self) -> Option<Result<Arc<dyn FileIo>>> {
        Some(Ok(self.weak_self.upgrade().unwrap()))
    }
}

impl CharDevice for MemDevice {
    fn name(&self) -> &str {
        self.file.name()
    }

    fn id(&self) -> DeviceId {
        self.id
    }

    fn as_device(&self) -> Arc<dyn Device> {
        self.weak_self.upgrade().unwrap()
    }
}

static MEM_MAJOR: Once<MajorIdOwner> = Once::new();

pub(super) fn init_in_first_kthread() {
    MEM_MAJOR.call_once(|| acquire_major(MajorId::new(1)).unwrap());

    register(MemDevice::new(MemFile::Full)).unwrap();
    register(MemDevice::new(MemFile::Null)).unwrap();
    register(MemDevice::new(MemFile::Random)).unwrap();
    register(MemDevice::new(MemFile::Urandom)).unwrap();
    register(MemDevice::new(MemFile::Zero)).unwrap();
}
