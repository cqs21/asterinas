// SPDX-License-Identifier: MPL-2.0

use alloc::{
    string::ToString,
    sync::{Arc, Weak},
};

use aster_systree::{
    inherit_sys_branch_node, inherit_sys_symlink_node, AttrLessBranchNodeFields, Result,
    SymlinkNodeFields, SysBranchNode, SysObj, SysPerms, SysStr,
};
use inherit_methods_macro::inherit_methods;

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
    target: Weak<dyn SysBranchNode>,
    field: SymlinkNodeFields<Self>,
}

inherit_sys_symlink_node!(DevSymlink, field);

impl DevSymlink {
    pub fn new(name: &str, target: &Arc<dyn SysBranchNode>) -> Arc<Self> {
        let name = SysStr::from(name.to_string());
        let target_path = target.path().to_string();
        Arc::new_cyclic(|weak_self| Self {
            target: Arc::downgrade(target),
            field: SymlinkNodeFields::new(name, target_path, weak_self.clone()),
        })
    }

    pub fn target(&self) -> Option<Arc<dyn SysBranchNode>> {
        let target = self.target.upgrade();
        if target.is_some() {
            return target;
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
    fields: AttrLessBranchNodeFields<dyn SysBranchNode, Self>,
}

inherit_sys_branch_node!(DevicesNode, fields, {
    fn perms(&self) -> SysPerms {
        SysPerms::DEFAULT_RW_PERMS
    }
});

#[inherit_methods(from = "self.fields")]
impl DevicesNode {
    pub fn child(&self, name: &str) -> Option<Arc<dyn SysBranchNode>>;
    pub fn add_child(&self, new_child: Arc<dyn SysBranchNode>) -> Result<()>;
    pub fn remove_child(&self, child_name: &str) -> Result<Arc<dyn SysBranchNode>>;
}

impl DevicesNode {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            fields: AttrLessBranchNodeFields::new(SysStr::from("devices"), weak_self.clone()),
        })
    }
}
