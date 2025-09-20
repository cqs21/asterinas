// SPDX-License-Identifier: MPL-2.0// SPDX-License-Identifier: MPL-2.0

use alloc::collections::{btree_map::BTreeMap, linked_list::LinkedList};
use core::ops::Range;

use ostd::{sync::RwLock, Error};

const CHRDEV_MAJOR_MAX: u32 = 512;
const CHRDEV_MINORS_MAX: u32 = 1 << super::id::MINOR_BITS;

const CHRDEV_FIRST_DYNAMIC_MAJOR_START: u32 = 234;
const CHRDEV_FIRST_DYNAMIC_MAJOR_END: u32 = 254;
const CHRDEV_SECOND_DYNAMIC_MAJOR_START: u32 = 384;
const CHRDEV_SECOND_DYNAMIC_MAJOR_END: u32 = 511;

static ID_MANAGER: RwLock<BTreeMap<u32, LinkedList<Range<u32>>>> = RwLock::new(BTreeMap::new());

pub fn register_device_ids(major: u32, minors: &Range<u32>) -> Result<u32, Error> {
    if major >= CHRDEV_MAJOR_MAX || minors.end >= CHRDEV_MINORS_MAX {
        return Err(Error::InvalidArgs);
    }

    let mut manager = ID_MANAGER.write();
    if major == 0 {
        for id in (CHRDEV_FIRST_DYNAMIC_MAJOR_START..CHRDEV_FIRST_DYNAMIC_MAJOR_END + 1).rev() {
            if !manager.contains_key(&id) {
                let mut list = LinkedList::new();
                list.push_back(minors.clone());
                manager.insert(id, list);
                return Ok(id);
            }
        }
        for id in (CHRDEV_SECOND_DYNAMIC_MAJOR_START..CHRDEV_SECOND_DYNAMIC_MAJOR_END + 1).rev() {
            if !manager.contains_key(&id) {
                let mut list = LinkedList::new();
                list.push_back(minors.clone());
                manager.insert(id, list);
                return Ok(id);
            }
        }
        return Err(Error::NotEnoughResources);
    }

    if !manager.contains_key(&major) {
        let mut list = LinkedList::new();
        list.push_back(minors.clone());
        manager.insert(major, list);
        return Ok(major);
    }

    let mut cursor = manager.get_mut(&major).unwrap().cursor_front_mut();
    while let Some(current) = cursor.current() {
        if minors.end <= current.start {
            cursor.insert_before(minors.clone());
            return Ok(major);
        }
        if minors.start >= current.end {
            cursor.move_next();
            continue;
        }
        return Err(Error::NotEnoughResources);
    }
    cursor.insert_before(minors.clone());

    Ok(major)
}

pub fn unregister_device_ids(major: u32, minors: &Range<u32>) -> Result<(), Error> {
    let mut manager = ID_MANAGER.write();
    if !manager.contains_key(&major) {
        return Ok(());
    }

    let list = manager.get_mut(&major).unwrap();
    let mut cursor = list.cursor_front_mut();
    while let Some(current) = cursor.current() {
        if minors == current {
            cursor.remove_current();
            break;
        }
        cursor.move_next();
    }

    if list.is_empty() {
        let _ = manager.remove(&major);
    }

    Ok(())
}
