// SPDX-License-Identifier: MPL-2.0

use alloc::{format, string::ToString, sync::Weak};

use aster_systree::{
    inherit_sys_branch_node, AttrLessBranchNodeFields, BranchNodeFields, Error, Result,
    SysAttrSetBuilder, SysBranchNode, SysObj, SysPerms, SysStr,
};
use inherit_methods_macro::inherit_methods;
use ostd::{
    mm::{FallibleVmWrite, VmWriter},
    sync::Mutex,
};

use crate::{prelude::*, BlockDevice};

#[derive(Debug)]
pub(crate) struct DeviceManager {
    devices: Mutex<BTreeMap<String, Arc<dyn BlockDevice>>>,
    fields: AttrLessBranchNodeFields<dyn SysBranchNode, Self>,
}

impl DeviceManager {
    pub(crate) fn new() -> Arc<Self> {
        let name = SysStr::from("block");
        let device_manager = Arc::new_cyclic(|weak_self| Self {
            devices: Mutex::new(BTreeMap::new()),
            fields: AttrLessBranchNodeFields::new(name, weak_self.clone()),
        });
        aster_systree::singleton()
            .root()
            .add_child(device_manager.clone())
            .unwrap();
        device_manager
    }

    pub(crate) fn register_device(&self, device: Arc<dyn BlockDevice>) {
        let sysnode = device.sysnode();
        self.devices
            .lock()
            .insert(sysnode.name().to_string(), device);
        self.fields.add_child(sysnode).unwrap();
    }

    pub(crate) fn get_device(&self, name: &str) -> Option<Arc<dyn BlockDevice>> {
        self.devices.lock().get(name).cloned()
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

/// A `SysTree` node of the block device under `/sys/block`.
///
/// This struct can represent:
/// - A device node, e.g., `/sys/block/vda`
/// - A partition node, e.g., `/sys/block/vda/vda1`
#[derive(Debug)]
pub struct BlockSysNode {
    pub common_info: BlockCommonInfo,
    pub extra_info: BlockExtraInfo,
    fields: BranchNodeFields<dyn SysObj, Self>,
}

/// Common information shared by both block devices and partitions.
#[derive(Debug)]
pub struct BlockCommonInfo {
    pub major: u32,
    pub minor: u32,
    pub size: u64,
}

/// Additional information that distinguishes between block devices and partitions.
#[derive(Debug)]
pub enum BlockExtraInfo {
    Device,
    Partition(PartitionInfo),
}

/// Partition-specific information for block device partitions.
#[derive(Debug)]
pub struct PartitionInfo {
    pub id: u32,
    pub start: u64,
}

impl BlockSysNode {
    pub fn new(
        name: SysStr,
        common_info: BlockCommonInfo,
        extra_info: BlockExtraInfo,
    ) -> Arc<Self> {
        let mut builder = SysAttrSetBuilder::new();
        // Add common attributes.
        builder.add(SysStr::from("dev"), SysPerms::DEFAULT_RO_ATTR_PERMS);
        builder.add(SysStr::from("size"), SysPerms::DEFAULT_RO_ATTR_PERMS);
        builder.add(SysStr::from("uevent"), SysPerms::DEFAULT_RW_ATTR_PERMS);

        // Add extra attributes.
        match &extra_info {
            BlockExtraInfo::Device => {}
            BlockExtraInfo::Partition(_) => {
                builder.add(SysStr::from("partition"), SysPerms::DEFAULT_RO_ATTR_PERMS);
                builder.add(SysStr::from("start"), SysPerms::DEFAULT_RO_ATTR_PERMS);
            }
        }

        let attrs = builder.build().expect("Failed to build attribute set");
        Arc::new_cyclic(|weak_self| {
            let fields = BranchNodeFields::new(name, attrs, weak_self.clone());
            BlockSysNode {
                common_info,
                extra_info,
                fields,
            }
        })
    }

    fn read(&self, attr: &str) -> Result<String> {
        // Try read common info
        let value = match attr {
            "dev" => Ok(format!(
                "{}:{}\n",
                self.common_info.major, self.common_info.minor
            )),
            "size" => Ok(format!("{}\n", self.common_info.size)),
            _ => Err(Error::NotFound),
        };
        if value.is_ok() {
            return value;
        }

        // Try read extra info
        let value = match &self.extra_info {
            BlockExtraInfo::Partition(partition) => match attr {
                "partition" => format!("{}\n", partition.id),
                "start" => format!("{}\n", partition.start),
                _ => return Err(Error::NotFound),
            },
            BlockExtraInfo::Device => return Err(Error::NotFound),
        };

        Ok(value)
    }
}

#[inherit_methods(from = "self.fields")]
impl BlockSysNode {
    pub fn add_child(&self, new_child: Arc<dyn SysObj>) -> Result<()>;
    pub fn weak_self(&self) -> &Weak<Self>;
}

inherit_sys_branch_node!(BlockSysNode, fields, {
    fn perms(&self) -> SysPerms {
        SysPerms::DEFAULT_RW_PERMS
    }

    fn read_attr(&self, name: &str, writer: &mut VmWriter) -> Result<usize> {
        // Check if attribute exists
        if !self.fields.attr_set().contains(name) {
            return Err(Error::NotFound);
        }

        let attr = self.fields.attr_set().get(name).unwrap();
        // Check if attribute is readable
        if !attr.perms().can_read() {
            return Err(Error::PermissionDenied);
        }

        let value = self.read(name)?;

        // Write the value to the provided writer
        writer
            .write_fallible(&mut (value.as_bytes()).into())
            .map_err(|_| Error::AttributeError)
    }
});
