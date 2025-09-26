// SPDX-License-Identifier: MPL-2.0

use alloc::collections::btree_set::BTreeSet;

use ostd::{sync::RwLock, Error};

const BLOCK_MAJOR_MAX: u32 = 512;
const BLOCK_LAST_DYNAMIC_MAJOR: u32 = 254;

static BLOCK_MAJORS: RwLock<BTreeSet<u32>> = RwLock::new(BTreeSet::new());

pub(crate) fn register_device_ids(major: u32) -> Result<u32, Error> {
    if major >= BLOCK_MAJOR_MAX {
        return Err(Error::InvalidArgs);
    }

    let mut majors = BLOCK_MAJORS.write();
    if major == 0 {
        for id in (1..BLOCK_LAST_DYNAMIC_MAJOR + 1).rev() {
            if majors.insert(id) {
                return Ok(id);
            }
        }
        return Err(Error::NotEnoughResources);
    }

    if majors.insert(major) {
        return Ok(major);
    }
    return Err(Error::NotEnoughResources);
}

pub(crate) fn unregister_device_ids(major: u32) {
    let _ = BLOCK_MAJORS.write().remove(&major);
}
