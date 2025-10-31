// SPDX-License-Identifier: MPL-2.0

use alloc::{format, string::ToString, sync::Weak};

use aster_systree::{
    inherit_sys_branch_node, inherit_sys_symlink_node, AttrLessBranchNodeFields, SymlinkNodeFields,
    SysObj, SysPerms, SysStr,
};
use ostd::sync::Mutex;

use crate::{prelude::*, BlockDevice};

#[derive(Debug)]
pub(crate) struct DeviceManager {
    devices: Mutex<BTreeMap<String, Arc<dyn BlockDevice>>>,
    fields: AttrLessBranchNodeFields<BlockSymlink, Self>,
}

impl DeviceManager {
    pub(crate) fn new() -> Arc<Self> {
        let name = SysStr::from("block");
        let device_manager = Arc::new_cyclic(|weak_self| Self {
            devices: Mutex::new(BTreeMap::new()),
            fields: AttrLessBranchNodeFields::new(name, weak_self.clone()),
        });
        aster_systree::primary_tree()
            .root()
            .add_child(device_manager.clone())
            .unwrap();
        device_manager
    }

    pub(crate) fn register_device(&self, device: Arc<dyn BlockDevice>) {
        if device.is_partition() {
            return;
        }

        let name = device.sysnode().name().to_string();
        self.fields.add_child(BlockSymlink::new(&device)).unwrap();
        self.devices.lock().insert(name, device);
    }

    pub(crate) fn get_device(&self, name: &str) -> Option<Arc<dyn BlockDevice>> {
        let devices = self.devices.lock();
        let (key, device) = devices.iter().find(|(key, _)| name.contains(*key))?;

        if key == name {
            return Some(device.clone());
        }

        let partitions = device.partitions()?;
        partitions.into_iter().find(|p| p.sysnode().name() == name)
    }

    pub(crate) fn all_devices(&self) -> Vec<Arc<dyn BlockDevice>> {
        self.devices.lock().values().cloned().collect()
    }
}

inherit_sys_branch_node!(DeviceManager, fields, {
    fn perms(&self) -> SysPerms {
        SysPerms::DEFAULT_RW_PERMS
    }
});

/// A symlink to a block device in `/sys/devices`.
#[derive(Debug)]
pub struct BlockSymlink {
    device: Weak<dyn BlockDevice>,
    field: SymlinkNodeFields<Self>,
}

inherit_sys_symlink_node!(BlockSymlink, field);

impl BlockSymlink {
    /// Creates a new `BlockSymlink` pointing to the specified device.
    pub fn new(device: &Arc<dyn BlockDevice>) -> Arc<Self> {
        let name = device.sysnode().name().to_string();
        let target_path = format!("../devices/{}", name);
        Arc::new_cyclic(|weak_self| Self {
            device: Arc::downgrade(device),
            field: SymlinkNodeFields::new(SysStr::from(name), target_path, weak_self.clone()),
        })
    }

    /// Retrieves the device associated with this symlink.
    pub fn device(&self) -> Option<Arc<dyn BlockDevice>> {
        let device = self.device.upgrade();
        if device.is_some() {
            return device;
        }

        if let Some(parent) = self.parent() {
            // Remove the invalid symlink from its parent.
            let _ = parent.remove_child(self.name());
        };

        None
    }
}
