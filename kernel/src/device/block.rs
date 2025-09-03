// SPDX-License-Identifier: MPL-2.0

use aster_block::{
    bio::{Bio, BioEnqueueError, BioStatus, SubmittedBio},
    sysnode::{BlockExtraInfo, BlockSysNode},
    BlockDevice, BlockDeviceMeta,
};
use aster_systree::{SysBranchNode, SysObj};
use aster_virtio::device::block::device::BlockDevice as VirtioBlockDevice;
use ostd::mm::VmIo;

use crate::{
    events::IoEvents,
    fs::{
        device::{add_node, Device, DeviceId, DeviceType},
        fs_resolver::FsResolver,
        inode_handle::FileIo,
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
    thread::kernel_thread::ThreadOptions,
};

pub(super) fn init_in_first_kthread() {
    for device in aster_block::all_devices() {
        let task_fn = move || {
            info!("spawn the virt-io-block thread");
            let virtio_block_device = device.downcast_ref::<VirtioBlockDevice>().unwrap();
            loop {
                virtio_block_device.handle_requests();
            }
        };
        ThreadOptions::new(task_fn).spawn();
    }
}

pub(super) fn init_in_first_process(fs_resolver: &FsResolver) -> Result<()> {
    for device in aster_block::all_devices() {
        let sysnode = device.sysnode();
        let name = sysnode.name().to_string();
        let mut partitions = Vec::new();
        for child_node in sysnode.children() {
            if child_node.name().starts_with(name.as_str()) {
                let weak_child = child_node
                    .as_any()
                    .downcast_ref::<BlockSysNode>()
                    .unwrap()
                    .weak_self()
                    .clone();
                let child = weak_child.upgrade().unwrap();

                let partition = Arc::new(BlockNode::new(&device, &child, None));
                add_node(partition.clone(), child.name(), fs_resolver)?;
                partitions.push(partition);
            }
        }

        let device = Arc::new(BlockNode::new(&device, &sysnode, Some(partitions)));
        add_node(device.clone(), &name, fs_resolver)?;
        BLOCK_REGISTRY.lock().push(device);
    }

    Ok(())
}

pub fn find_block_device(abs_path: &str) -> Option<Arc<dyn BlockDevice>> {
    let device_name = abs_path.trim_start_matches("/dev/");
    for block in BLOCK_REGISTRY.lock().iter() {
        if device_name == block.name() {
            return block.device();
        }

        if device_name.starts_with(block.name()) {
            return block
                .partition(device_name)
                .map(|p| p as Arc<dyn BlockDevice>);
        };
    }

    None
}

static BLOCK_REGISTRY: Mutex<Vec<Arc<BlockNode>>> = Mutex::new(Vec::new());

#[derive(Debug, Clone)]
struct BlockNode {
    id: DeviceId,
    name: String,
    start: usize,
    size: usize,
    device: Weak<dyn BlockDevice>,
    sysndoe: Weak<BlockSysNode>,
    partitions: Option<Vec<Arc<BlockNode>>>,
}

impl BlockNode {
    pub fn new(
        device: &Arc<dyn BlockDevice>,
        sysnode: &Arc<BlockSysNode>,
        partitions: Option<Vec<Arc<BlockNode>>>,
    ) -> Self {
        let start = match &sysnode.extra_info {
            BlockExtraInfo::Device => 0,
            BlockExtraInfo::Partition(partition) => partition.start as usize,
        };

        Self {
            id: DeviceId::new(sysnode.common_info.major, sysnode.common_info.minor),
            name: sysnode.name().to_string(),
            start,
            size: sysnode.common_info.size as usize,
            device: Arc::downgrade(device),
            sysndoe: Arc::downgrade(sysnode),
            partitions,
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn device(&self) -> Option<Arc<dyn BlockDevice>> {
        self.device.upgrade()
    }

    pub fn partition(&self, name: &str) -> Option<Arc<BlockNode>> {
        let Some(partitions) = &self.partitions else {
            return None;
        };

        partitions.iter().find(|p| p.name == name).cloned()
    }
}

impl Device for BlockNode {
    fn id(&self) -> DeviceId {
        self.id
    }

    fn type_(&self) -> super::DeviceType {
        DeviceType::Block
    }

    fn open(&self) -> Result<Option<Arc<dyn FileIo>>> {
        Ok(Some(Arc::new(self.clone())))
    }
}

impl FileIo for BlockNode {
    fn read(&self, writer: &mut VmWriter) -> Result<usize> {
        let Some(device) = self.device() else {
            return_errno_with_message!(Errno::EIO, "device is gone");
        };

        let total = writer.avail();
        device.read(self.start, writer)?;
        let avail = writer.avail();
        Ok(total - avail)
    }

    fn write(&self, reader: &mut VmReader) -> Result<usize> {
        let Some(device) = self.device() else {
            return_errno_with_message!(Errno::EIO, "device is gone");
        };

        let total = reader.remain();
        device.write(self.start, reader)?;
        let remain = reader.remain();
        Ok(total - remain)
    }
}

impl Pollable for BlockNode {
    fn poll(&self, _mask: IoEvents, _poller: Option<&mut PollHandle>) -> IoEvents {
        IoEvents::empty()
    }
}

impl BlockDevice for BlockNode {
    fn enqueue(&self, bio: SubmittedBio) -> core::result::Result<(), BioEnqueueError> {
        let Some(device) = self.device.upgrade() else {
            bio.complete(BioStatus::IoError);
            return Ok(());
        };

        let start_sid = bio.sid_range().start + self.start as u64;
        let segments = Vec::from_iter(bio.segments().iter().cloned());
        let new_bio = Bio::new(bio.type_(), start_sid, segments, None);

        let Ok(status) = new_bio.submit_and_wait(device.as_ref()) else {
            bio.complete(BioStatus::IoError);
            return Ok(());
        };

        bio.complete(status);
        Ok(())
    }

    fn metadata(&self) -> BlockDeviceMeta {
        let mut metadata = BlockDeviceMeta::default();
        let Some(device) = self.device.upgrade() else {
            return metadata;
        };

        metadata.max_nr_segments_per_bio = device.metadata().max_nr_segments_per_bio;
        metadata.nr_sectors = self.size;
        metadata
    }

    fn sysnode(&self) -> Arc<BlockSysNode> {
        self.sysndoe.upgrade().unwrap()
    }
}
