// SPDX-License-Identifier: MPL-2.0

//! The devices of Asterinas.
#![no_std]
#![deny(unsafe_code)]
#![feature(linked_list_cursors)]
#![feature(trait_upcasting)]

extern crate alloc;

mod block;
mod char;
mod id;
mod sysnode;

use alloc::{format, string::ToString, sync::Arc};
use core::{any::Any, ops::Range};

use aster_systree::SysBranchNode;
use component::{init_component, ComponentInitError};
pub use id::{
    register_device_ids, unregister_device_ids, DeviceId, DeviceIdAllocator, DeviceType,
    MinorIdAllocator, NoConflictMinorIdAllocator, MAJOR_BITS, MINOR_BITS,
};
use spin::Once;
pub use sysnode::{DevNode, DevSymlink, DevSymlinks, DevicesNode};

pub trait Device {
    fn type_(&self) -> DeviceType;
    fn id(&self) -> Option<DeviceId>;
    fn sysnode(&self) -> Arc<dyn SysBranchNode>;
}

pub fn add_device(device: Arc<dyn Device>) {
    let sysnode = device.sysnode();
    if sysnode.parent().is_none() {
        DEVICES_NODE
            .get()
            .unwrap()
            .add_child(sysnode.clone())
            .unwrap();
    }

    let Some(id) = device.id() else {
        return;
    };
    let sys_name = match device.type_() {
        DeviceType::Block => "block",
        DeviceType::Character => "char",
        DeviceType::Unknown => return,
    };
    let node_name = format!("{}:{}", id.major, id.minor);
    let symlink = DEV_NODE.get().unwrap().child(sys_name).unwrap();
    let new_node = DevSymlink::new(node_name.as_str(), &sysnode);
    symlink.add_child(new_node).unwrap();
}

pub fn del_device(device: Arc<dyn Device>) {
    let sysnode = device.sysnode();
    if sysnode.parent().is_none() {
        DEVICES_NODE
            .get()
            .unwrap()
            .remove_child(sysnode.name())
            .unwrap();
    }

    let Some(id) = device.id() else {
        return;
    };
    let sys_name = match device.type_() {
        DeviceType::Block => "block",
        DeviceType::Character => "char",
        DeviceType::Unknown => return,
    };
    let node_name = format!("{}:{}", id.major, id.minor);
    let symlink = DEV_NODE.get().unwrap().child(sys_name).unwrap();
    let _ = symlink.remove_child(&node_name);
}

static DEV_NODE: Once<Arc<DevNode>> = Once::new();

static DEVICES_NODE: Once<Arc<DevicesNode>> = Once::new();

#[init_component]
fn init() -> Result<(), ComponentInitError> {
    let sys_tree = aster_systree::singleton().root();

    let devices_node = DevicesNode::new();
    sys_tree.add_child(devices_node.clone()).unwrap();
    DEVICES_NODE.call_once(|| devices_node);

    let dev_node = DevNode::new();
    dev_node.add_child(DevSymlinks::new("block")).unwrap();
    dev_node.add_child(DevSymlinks::new("char")).unwrap();
    sys_tree.add_child(dev_node.clone()).unwrap();
    DEV_NODE.call_once(|| dev_node);

    Ok(())
}
