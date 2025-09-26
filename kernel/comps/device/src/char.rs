// SPDX-License-Identifier: MPL-2.0// SPDX-License-Identifier: MPL-2.0

use alloc::collections::{btree_map::BTreeMap, linked_list::LinkedList};
use core::ops::Range;

use ostd::{sync::RwLock, Error};

const CHAR_MAJOR_MAX: u32 = 512;
const CHAR_MINORS_MAX: u32 = 1 << super::id::MINOR_BITS;

const CHAR_FIRST_DYNAMIC_MAJOR_START: u32 = 234;
const CHAR_FIRST_DYNAMIC_MAJOR_END: u32 = 254;
const CHAR_SECOND_DYNAMIC_MAJOR_START: u32 = 384;
const CHAR_SECOND_DYNAMIC_MAJOR_END: u32 = 511;

static CHAR_MAJORS: RwLock<BTreeMap<u32, LinkedList<Range<u32>>>> = RwLock::new(BTreeMap::new());

pub(crate) fn register_device_ids(major: u32, minors: &Range<u32>) -> Result<u32, Error> {
    if major >= CHAR_MAJOR_MAX || minors.end > CHAR_MINORS_MAX {
        return Err(Error::InvalidArgs);
    }

    let mut majors = CHAR_MAJORS.write();
    if major == 0 {
        for id in (CHAR_FIRST_DYNAMIC_MAJOR_START..CHAR_FIRST_DYNAMIC_MAJOR_END + 1).rev() {
            if !majors.contains_key(&id) {
                let mut list = LinkedList::new();
                list.push_back(minors.clone());
                majors.insert(id, list);
                return Ok(id);
            }
        }
        for id in (CHAR_SECOND_DYNAMIC_MAJOR_START..CHAR_SECOND_DYNAMIC_MAJOR_END + 1).rev() {
            if !majors.contains_key(&id) {
                let mut list = LinkedList::new();
                list.push_back(minors.clone());
                majors.insert(id, list);
                return Ok(id);
            }
        }
        return Err(Error::NotEnoughResources);
    }

    if !majors.contains_key(&major) {
        let mut list = LinkedList::new();
        list.push_back(minors.clone());
        majors.insert(major, list);
        return Ok(major);
    }

    let mut cursor = majors.get_mut(&major).unwrap().cursor_front_mut();
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

pub(crate) fn unregister_device_ids(major: u32, minors: &Range<u32>) {
    let mut majors = CHAR_MAJORS.write();
    if !majors.contains_key(&major) {
        return;
    }

    let list = majors.get_mut(&major).unwrap();
    let mut cursor = list.cursor_front_mut();
    while let Some(current) = cursor.current() {
        if minors == current {
            cursor.remove_current();
            break;
        }
        cursor.move_next();
    }

    if list.is_empty() {
        let _ = majors.remove(&major);
    }
}
