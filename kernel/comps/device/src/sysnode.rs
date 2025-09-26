// SPDX-License-Identifier: MPL-2.0

use alloc::{
    collections::btree_map::BTreeMap,
    format,
    string::ToString,
    sync::{Arc, Weak},
};

use aster_systree::{
    inherit_sys_branch_node, inherit_sys_symlink_node, AttrLessBranchNodeFields, Result,
    SymlinkNodeFields, SysBranchNode, SysObj, SysPerms, SysStr,
};
use inherit_methods_macro::inherit_methods;
use ostd::sync::RwLock;

use super::Device;

/// The `dev` node in sysfs.
#[derive(Debug)]
pub struct DevNode {
    fields: AttrLessBranchNodeFields<DevSymlinks, Self>,
}

inherit_sys_branch_node!(DevNode, fields, {
    fn perms(&self) -> SysPerms {
        SysPerms::DEFAULT_RW_PERMS
    }
});

#[inherit_methods(from = "self.fields")]
impl DevNode {
    pub fn init_parent(&self, parent: Weak<dyn SysBranchNode>);
    pub fn weak_self(&self) -> &Weak<Self>;
    pub fn child(&self, name: &str) -> Option<Arc<DevSymlinks>>;
    pub fn add_child(&self, new_child: Arc<DevSymlinks>) -> Result<()>;
    pub fn remove_child(&self, child_name: &str) -> Result<Arc<DevSymlinks>>;
}

impl DevNode {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            fields: AttrLessBranchNodeFields::new(SysStr::from("dev"), weak_self.clone()),
        })
    }
}

#[derive(Debug)]
pub struct DevSymlinks {
    fields: AttrLessBranchNodeFields<DevSymlink, Self>,
}

inherit_sys_branch_node!(DevSymlinks, fields, {
    fn perms(&self) -> SysPerms {
        SysPerms::DEFAULT_RW_PERMS
    }
});

#[inherit_methods(from = "self.fields")]
impl DevSymlinks {
    pub fn init_parent(&self, parent: Weak<dyn SysBranchNode>);
    pub fn weak_self(&self) -> &Weak<Self>;
    pub fn child(&self, name: &str) -> Option<Arc<DevSymlink>>;
    pub fn add_child(&self, new_child: Arc<DevSymlink>) -> Result<()>;
    pub fn remove_child(&self, child_name: &str) -> Result<Arc<DevSymlink>>;
}

impl DevSymlinks {
    pub(crate) fn new(name: &str) -> Arc<Self> {
        let name = SysStr::from(name.to_string());
        Arc::new_cyclic(|weak_self| Self {
            fields: AttrLessBranchNodeFields::new(name, weak_self.clone()),
        })
    }
}

#[derive(Debug)]
pub struct DevSymlink {
    device: Weak<dyn Device>,
    field: SymlinkNodeFields<Self>,
}

inherit_sys_symlink_node!(DevSymlink, field);

impl DevSymlink {
    pub fn new(name: &str, device: &Arc<dyn Device>) -> Arc<Self> {
        let name = SysStr::from(name.to_string());
        let target_path = format!("../..{}", device.sysnode().path());
        Arc::new_cyclic(|weak_self| Self {
            device: Arc::downgrade(device),
            field: SymlinkNodeFields::new(name, target_path, weak_self.clone()),
        })
    }

    pub fn device(&self) -> Option<Arc<dyn Device>> {
        let device = self.device.upgrade();
        if device.is_some() {
            return device;
        }

        if let Some(parent) = self.parent() {
            // Remove the invalid symlink from its parent.
            let _ = parent.remove_child(self.name());
        };

        return None;
    }
}

/// The `devices` node in sysfs.
#[derive(Debug)]
pub struct DevicesNode {
    devices: RwLock<BTreeMap<SysStr, Arc<dyn Device>>>,
    fields: AttrLessBranchNodeFields<dyn SysBranchNode, Self>,
}

inherit_sys_branch_node!(DevicesNode, fields, {
    fn perms(&self) -> SysPerms {
        SysPerms::DEFAULT_RW_PERMS
    }
});

#[inherit_methods(from = "self.fields")]
impl DevicesNode {
    pub fn init_parent(&self, parent: Weak<dyn SysBranchNode>);
    pub fn weak_self(&self) -> &Weak<Self>;
    pub fn child(&self, name: &str) -> Option<Arc<dyn SysBranchNode>>;
    pub fn add_child(&self, new_child: Arc<dyn SysBranchNode>) -> Result<()>;
    pub fn remove_child(&self, child_name: &str) -> Result<Arc<dyn SysBranchNode>>;
}

impl DevicesNode {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            devices: RwLock::new(BTreeMap::new()),
            fields: AttrLessBranchNodeFields::new(SysStr::from("devices"), weak_self.clone()),
        })
    }

    pub fn add_device(&self, device: Arc<dyn Device>) {
        let sysnode = device.sysnode();
        self.devices.write().insert(sysnode.name().clone(), device);
        self.add_child(sysnode).unwrap();
    }

    pub fn remove_device(&self, name: &str) {
        let Some(device) = self.devices.write().remove(name) else {
            return;
        };

        self.remove_child(device.sysnode().name()).unwrap();
    }
}
