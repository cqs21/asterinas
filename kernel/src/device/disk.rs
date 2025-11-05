// SPDX-License-Identifier: MPL-2.0

use aster_block::BlockDevice;
use aster_device::{Device, DeviceId, DeviceType};
use aster_systree::SysBranchNode;
use aster_virtio::device::block::device::BlockDevice as VirtIoBlockDevice;
use ostd::mm::VmIo;

use crate::{
    events::IoEvents,
    fs::{
        device::{add_device, DeviceFile},
        inode_handle::FileIo,
        utils::StatusFlags,
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
};

pub(super) fn init_in_first_kthread() {
    for device in aster_block::all_devices() {
        let task_fn = move || {
            info!("spawn the virt-io-block thread");
            let virtio_block_device = device.downcast_ref::<VirtIoBlockDevice>().unwrap();
            loop {
                virtio_block_device.handle_requests();
            }
        };
        crate::ThreadOptions::new(task_fn).spawn();
    }
}

pub(super) fn init_in_first_process() {
    for device in aster_block::all_devices() {
        add_device(BlockFile::new(device.clone()));

        let Some(partitions) = device.partitions() else {
            continue;
        };

        for partition in partitions {
            add_device(BlockFile::new(partition));
        }
    }
}

#[derive(Debug)]
struct BlockFile {
    device: Arc<dyn BlockDevice>,
    weak_self: Weak<Self>,
}

impl BlockFile {
    fn new(device: Arc<dyn BlockDevice>) -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            device,
            weak_self: weak_self.clone(),
        })
    }
}

impl Device for BlockFile {
    fn type_(&self) -> DeviceType {
        self.device.type_()
    }

    fn id(&self) -> Option<DeviceId> {
        self.device.id()
    }

    fn sysnode(&self) -> Arc<dyn SysBranchNode> {
        self.device.sysnode()
    }
}

impl FileIo for BlockFile {
    fn read(&self, writer: &mut VmWriter, _status_flags: StatusFlags) -> Result<usize> {
        let total = writer.avail();
        self.device.read(0, writer)?;
        let avail = writer.avail();
        Ok(total - avail)
    }

    fn write(&self, reader: &mut VmReader, _status_flags: StatusFlags) -> Result<usize> {
        let total = reader.remain();
        self.device.write(0, reader)?;
        let remain = reader.remain();
        Ok(total - remain)
    }
}

impl Pollable for BlockFile {
    fn poll(&self, _: IoEvents, _: Option<&mut PollHandle>) -> IoEvents {
        IoEvents::empty()
    }
}

impl DeviceFile for BlockFile {
    fn open(&self) -> Option<Result<Arc<dyn FileIo>>> {
        Some(Ok(self.weak_self.upgrade().unwrap()))
    }
}
