// SPDX-License-Identifier: MPL-2.0

use alloc::collections::btree_set::BTreeSet;

use ostd::{sync::RwLock, Error};

const BLKDEV_MAJOR_MAX: u32 = 512;
const BLKDEV_LAST_DYNAMIC_MAJOR: u32 = 254;

static ID_MANAGER: RwLock<BTreeSet<u32>> = RwLock::new(BTreeSet::new());

pub fn register_device_ids(major: u32) -> Result<u32, Error> {
    if major >= BLKDEV_MAJOR_MAX {
        return Err(Error::InvalidArgs);
    }

    let mut manager = ID_MANAGER.write();
    if major == 0 {
        for id in (1..BLKDEV_LAST_DYNAMIC_MAJOR + 1).rev() {
            if manager.insert(id) {
                return Ok(id);
            }
        }
    }

    if manager.insert(major) {
        return Ok(major);
    }

    Err(Error::NotEnoughResources)
}

pub fn unregister_device_ids(major: u32) -> Result<(), Error> {
    let _ = ID_MANAGER.write().remove(&major);
    Ok(())
}
