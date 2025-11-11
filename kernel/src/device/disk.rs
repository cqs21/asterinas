// SPDX-License-Identifier: MPL-2.0

use aster_block::BlockDevice;
use aster_virtio::device::block::device::BlockDevice as VirtIoBlockDevice;
use ostd::mm::VmIo;

use crate::{
    events::IoEvents,
    fs::{
        device::{add_node, Device, DeviceId, DeviceType},
        fs_resolver::FsResolver,
        inode_handle::FileIo,
        utils::StatusFlags,
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
    thread::kernel_thread::ThreadOptions,
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
        ThreadOptions::new(task_fn).spawn();
    }
}

pub(super) fn init_in_first_process(fs_resolver: &FsResolver) -> Result<()> {
    for device in aster_block::all_devices() {
        if let Some(partitions) = device.partitions() {
            for partition in partitions {
                add_node(BlockFile::new(&partition), partition.name(), fs_resolver)?;
            }
        }

        add_node(BlockFile::new(&device), device.name(), fs_resolver)?;
    }

    Ok(())
}

#[derive(Debug)]
struct BlockFile {
    device: Arc<dyn BlockDevice>,
    weak_self: Weak<Self>,
}

impl BlockFile {
    fn new(device: &Arc<dyn BlockDevice>) -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            device: device.clone(),
            weak_self: weak_self.clone(),
        })
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

impl Device for BlockFile {
    fn type_(&self) -> DeviceType {
        DeviceType::Block
    }

    fn id(&self) -> DeviceId {
        let (major, minor) = self.device.id();
        DeviceId::new(major, minor)
    }

    fn open(&self) -> Option<Result<Arc<dyn FileIo>>> {
        Some(Ok(self.weak_self.upgrade().unwrap()))
    }
}
