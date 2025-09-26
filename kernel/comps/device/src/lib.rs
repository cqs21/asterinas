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

use alloc::{format, sync::Arc};
use core::{any::Any, fmt::Debug};

use aster_systree::SysBranchNode;
use component::{init_component, ComponentInitError};
pub use id::{
    register_device_ids, DeviceId, DeviceIdAllocator, DeviceType, MAJOR_BITS, MINOR_BITS,
};
use spin::Once;
pub use sysnode::{DevNode, DevSymlink, DevSymlinks, DevicesNode};

pub trait Device: Any + Debug + Send + Sync {
    fn device_type(&self) -> DeviceType;
    fn device_id(&self) -> Option<DeviceId>;
    fn sysnode(&self) -> Arc<dyn SysBranchNode>;
    fn as_any(&self) -> &dyn Any;
}

pub fn add_device(device: Arc<dyn Device>) {
    let sysnode = device.sysnode();
    if sysnode.parent().is_none() {
        DEVICES_NODE.get().unwrap().add_device(device.clone());
    }

    let sys_name = match device.device_type() {
        DeviceType::Block => "block",
        DeviceType::Char => "char",
        DeviceType::Other => return,
    };
    let Some(id) = device.device_id() else {
        return;
    };
    let dev_name = format!("{}:{}", id.major(), id.minor());
    let dev_symlink = DevSymlink::new(&dev_name, &device);
    DEV_NODE
        .get()
        .unwrap()
        .child(sys_name)
        .unwrap()
        .add_child(dev_symlink)
        .unwrap();
}

pub fn del_device(device: Arc<dyn Device>) {
    let sysnode = device.sysnode();
    let _ = DEVICES_NODE.get().unwrap().remove_child(sysnode.name());

    let sys_name = match device.device_type() {
        DeviceType::Block => "block",
        DeviceType::Char => "char",
        DeviceType::Other => return,
    };
    let Some(id) = device.device_id() else {
        return;
    };
    let dev_name = format!("{}:{}", id.major(), id.minor());
    let _ = DEV_NODE
        .get()
        .unwrap()
        .child(sys_name)
        .unwrap()
        .remove_child(&dev_name);
}

pub fn get_device(type_: DeviceType, id: DeviceId) -> Option<Arc<dyn Device>> {
    let dev_node = DEV_NODE.get().unwrap();
    let sys_name = match type_ {
        DeviceType::Block => "block",
        DeviceType::Char => "char",
        DeviceType::Other => return None,
    };

    let Some(dev_symlinks) = dev_node.child(sys_name) else {
        return None;
    };

    let node_name = format!("{}:{}", id.major(), id.minor());
    let Some(symlink) = dev_symlinks.child(&node_name) else {
        return None;
    };

    symlink.device()
}

pub fn all_devices() -> impl Iterator<Item = Arc<dyn Device>> {
    let dev_node = DEV_NODE.get().unwrap();
    let blocks = dev_node.child("block").unwrap().children();
    let chars = dev_node.child("char").unwrap().children();

    blocks
        .into_iter()
        .chain(chars.into_iter())
        .filter_map(|node| Arc::downcast::<DevSymlink>(node).unwrap().device())
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
