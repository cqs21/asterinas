// SPDX-License-Identifier: MPL-2.0

//! A subsystem for character devices (or char devices for short).

#![expect(dead_code)]

use device_id::{DeviceId, MajorId};

use crate::{
    fs::{
        device::{add_node, Device},
        fs_resolver::FsResolver,
    },
    prelude::*,
};

/// A character device.
pub trait CharDevice: Send + Sync + Debug {
    /// Returns the name of the char device.
    fn name(&self) -> &str;

    /// Returns the device ID.
    fn id(&self) -> DeviceId;

    /// Returns the char device as a `Device` object.
    fn as_device(&self) -> Arc<dyn Device>;
}

static DEVICE_REGISTRY: Mutex<BTreeMap<u32, Arc<dyn CharDevice>>> = Mutex::new(BTreeMap::new());

/// Registers a new char device.
pub fn register(device: Arc<dyn CharDevice>) -> Result<()> {
    let mut registry = DEVICE_REGISTRY.lock();
    let id = device.id().to_raw();
    if registry.contains_key(&id) {
        return_errno_with_message!(Errno::EEXIST, "char device already exists");
    }
    registry.insert(id, device);

    Ok(())
}

/// Unregisters an existing char device, returning the device if found.
pub fn unregister(id: DeviceId) -> Result<Arc<dyn CharDevice>> {
    DEVICE_REGISTRY
        .lock()
        .remove(&id.to_raw())
        .ok_or(Error::with_message(
            Errno::ENOENT,
            "char device does not exist",
        ))
}

/// Collects all char devices.
pub fn collect_all() -> Vec<Arc<dyn CharDevice>> {
    DEVICE_REGISTRY.lock().values().cloned().collect()
}

/// Looks up a char device of a given device ID.
pub fn lookup(id: DeviceId) -> Option<Arc<dyn CharDevice>> {
    DEVICE_REGISTRY.lock().get(&id.to_raw()).cloned()
}

/// The upper limit for char device major IDs (not included).
const MAJOR_MAX: u16 = 512;

/// The bottom of the first segment of free char majors.
const CHAR_FIRST_DYNAMIC_MAJOR_START: u16 = 234;

/// The top of the first segment of free char majors (included).
const CHAR_FIRST_DYNAMIC_MAJOR_END: u16 = 254;

/// The bottom of the second segment of free char majors.
const CHAR_SECOND_DYNAMIC_MAJOR_START: u16 = 384;

/// The top of the second segment of free char majors (included).
const CHAR_SECOND_DYNAMIC_MAJOR_END: u16 = 511;

static MAJORS: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// Acquires a major ID.
///
/// The returned `MajorIdOwner` object represents the ownership to the major ID.
/// Until the object is dropped, this major ID cannot be acquired via `acquire_major` or `allocate_major` again.
pub fn acquire_major(major: MajorId) -> Result<MajorIdOwner> {
    if major.get() == 0 || major.get() >= MAJOR_MAX {
        return_errno_with_message!(Errno::EINVAL, "invalid major ID");
    }

    if MAJORS.lock().insert(major.get()) {
        Ok(MajorIdOwner(major))
    } else {
        return_errno_with_message!(Errno::EEXIST, "major ID already acquired")
    }
}

/// Allocates a major ID.
///
/// The returned `MajorIdOwner` object represents the ownership to the major ID.
/// Until the object is dropped, this major ID cannot be acquired via `acquire_major` or `allocate_major` again.
pub fn allocate_major() -> Result<MajorIdOwner> {
    let mut majors = MAJORS.lock();

    for id in (CHAR_FIRST_DYNAMIC_MAJOR_START..CHAR_FIRST_DYNAMIC_MAJOR_END + 1).rev() {
        if majors.insert(id) {
            return Ok(MajorIdOwner(MajorId::new(id)));
        }
    }

    for id in (CHAR_SECOND_DYNAMIC_MAJOR_START..CHAR_SECOND_DYNAMIC_MAJOR_END + 1).rev() {
        if majors.insert(id) {
            return Ok(MajorIdOwner(MajorId::new(id)));
        }
    }

    return_errno_with_message!(Errno::ENOSPC, "no more major IDs available");
}

/// An owned major ID.
///
/// Each instances of this type will unregister the major ID when dropped.
pub struct MajorIdOwner(MajorId);

impl MajorIdOwner {
    /// Returns the major ID.
    pub fn get(&self) -> MajorId {
        self.0
    }
}

impl Drop for MajorIdOwner {
    fn drop(&mut self) {
        MAJORS.lock().remove(&self.0.get());
    }
}

pub(super) fn init_in_first_process(fs_resolver: &FsResolver) -> Result<()> {
    for device in collect_all() {
        add_node(device.as_device(), device.name(), fs_resolver)?;
    }

    Ok(())
}
