// SPDX-License-Identifier: MPL-2.0

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    format,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    fmt::Debug,
    sync::atomic::{AtomicU32, Ordering},
};

use aster_block::{
    alloc_extended_device_id,
    bio::{bio_segment_pool_init, BioEnqueueError, BioStatus, BioType, SubmittedBio},
    request_queue::{BioRequest, BioRequestSingleQueue},
    BlockDeviceMeta, PartitionInfo, PartitionNode, LEGACY_PARTITION_LIMIT,
};
use aster_device::{Device, DeviceId, DeviceIdAllocator, DeviceType};
use aster_systree::{
    inherit_sys_branch_node, BranchNodeFields, Error, SysAttrSetBuilder, SysBranchNode, SysObj,
    SysPerms, SysStr,
};
use aster_util::{mem_obj_slice::Slice, printer::VmPrinter};
use id_alloc::IdAlloc;
use inherit_methods_macro::inherit_methods;
use log::{debug, info};
use ostd::{
    arch::trap::TrapFrame,
    mm::{DmaDirection, DmaStream, FrameAllocOptions, HasSize, VmIo, VmWriter},
    sync::{Mutex, SpinLock},
    Pod,
};

use super::{BlockFeatures, VirtioBlockConfig, VirtioBlockFeature};
use crate::{
    device::{
        block::{ReqType, RespStatus},
        VirtioDeviceError,
    },
    queue::VirtQueue,
    transport::{ConfigManager, VirtioTransport},
    VIRTIO_BLOCK_ID_ALLOCATOR,
};

/// The number of minor device numbers allocated for each virtio disk,
/// including the whole disk and its partitions. If a disk has more than
/// 16 partitions, the extended major:minor numbers will be assigned.
const VIRTIO_DEVICE_MINORS: u32 = 16;

/// The number of virtio block devices, used to assign minor device numbers.
static NR_BLOCK_DEVICE: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
pub struct BlockDevice {
    device: Arc<DeviceInner>,
    /// The software staging queue.
    queue: BioRequestSingleQueue,
    partitions: Mutex<Option<Vec<Arc<PartitionNode>>>>,
    id: DeviceId,
    fields: BranchNodeFields<dyn SysBranchNode, Self>,
}

impl Device for BlockDevice {
    fn type_(&self) -> DeviceType {
        DeviceType::Block
    }

    fn id(&self) -> Option<DeviceId> {
        Some(self.id)
    }

    fn sysnode(&self) -> Arc<dyn SysBranchNode> {
        self.weak_self().upgrade().unwrap()
    }
}

inherit_sys_branch_node!(BlockDevice, fields, {
    fn perms(&self) -> SysPerms {
        SysPerms::DEFAULT_RW_PERMS
    }

    fn read_attr_at(
        &self,
        name: &str,
        offset: usize,
        writer: &mut VmWriter,
    ) -> aster_systree::Result<usize> {
        // Check if attribute exists
        if !self.fields.attr_set().contains(name) {
            return Err(Error::NotFound);
        }

        let attr = self.fields.attr_set().get(name).unwrap();
        // Check if attribute is readable
        if !attr.perms().can_read() {
            return Err(Error::PermissionDenied);
        }

        let mut printer = VmPrinter::new_skip(writer, offset);
        let res = match name {
            "dev" => writeln!(printer, "{}:{}", self.id.major(), self.id.minor()),
            "size" => writeln!(printer, "{}", self.device.config_manager.capacity_sectors()),
            _ => Ok(()),
        };
        res.map_err(|_| Error::AttributeError)?;

        Ok(printer.bytes_written())
    }
});

#[inherit_methods(from = "self.fields")]
impl BlockDevice {
    pub fn init_parent(&self, parent: Weak<dyn SysBranchNode>);
    pub fn weak_self(&self) -> &Weak<Self>;
    pub fn child(&self, name: &str) -> Option<Arc<dyn SysBranchNode>>;
    pub fn add_child(&self, new_child: Arc<dyn SysBranchNode>) -> aster_systree::Result<()>;
    pub fn remove_child(&self, child_name: &str) -> aster_systree::Result<Arc<dyn SysBranchNode>>;
}

impl BlockDevice {
    /// Returns the formatted device name.
    ///
    /// The device name starts at "vda". The 26th device is "vdz" and the 27th is "vdaa".
    /// The last one for two lettered suffix is "vdzz" which is followed by "vdaaa".
    fn formatted_device_name(mut index: u32) -> String {
        const VIRTIO_DISK_PREFIX: &str = "vd";

        let mut suffix = Vec::new();
        loop {
            suffix.push((b'a' + (index % 26) as u8) as char);
            index /= 26;
            if index == 0 {
                break;
            }
            index -= 1;
        }
        suffix.reverse();
        let mut name = String::from(VIRTIO_DISK_PREFIX);
        name.extend(suffix);
        name
    }

    /// Creates a new VirtIO-Block driver and registers it.
    pub(crate) fn init(transport: Box<dyn VirtioTransport>) -> Result<(), VirtioDeviceError> {
        let device = DeviceInner::init(transport)?;

        let index = NR_BLOCK_DEVICE.fetch_add(1, Ordering::Relaxed);
        let id = VIRTIO_BLOCK_ID_ALLOCATOR
            .get()
            .unwrap()
            .allocate(index * VIRTIO_DEVICE_MINORS)
            .unwrap();
        let name = Self::formatted_device_name(index);

        let mut builder = SysAttrSetBuilder::new();
        // Add common attributes.
        builder.add(SysStr::from("dev"), SysPerms::DEFAULT_RO_ATTR_PERMS);
        builder.add(SysStr::from("size"), SysPerms::DEFAULT_RO_ATTR_PERMS);
        builder.add(SysStr::from("uevent"), SysPerms::DEFAULT_RW_ATTR_PERMS);
        let attrs = builder.build().expect("Failed to build attribute set");

        let block_device = Arc::new_cyclic(|weak_self| BlockDevice {
            device,
            // Each bio request includes an additional 1 request and 1 response descriptor,
            // therefore this upper bound is set to (QUEUE_SIZE - 2).
            queue: BioRequestSingleQueue::with_max_nr_segments_per_bio(
                (DeviceInner::QUEUE_SIZE - 2) as usize,
            ),
            partitions: Mutex::new(None),
            id,
            fields: BranchNodeFields::new(SysStr::from(name), attrs, weak_self.clone()),
        });

        aster_block::register_device(block_device);

        bio_segment_pool_init();
        Ok(())
    }

    /// Dequeues a `BioRequest` from the software staging queue and
    /// processes the request.
    pub fn handle_requests(&self) {
        let request = self.queue.dequeue();
        info!("Handle Request: {:?}", request);
        match request.type_() {
            BioType::Read => self.device.read(request),
            BioType::Write => self.device.write(request),
            BioType::Flush => self.device.flush(request),
            BioType::Discard => todo!(),
        }
    }

    /// Negotiate features for the device specified bits 0~23
    pub(crate) fn negotiate_features(features: u64) -> u64 {
        let mut support_features = BlockFeatures::from_bits_truncate(features);
        support_features.remove(BlockFeatures::MQ);
        support_features.bits
    }
}

impl aster_block::BlockDevice for BlockDevice {
    fn enqueue(&self, bio: SubmittedBio) -> Result<(), BioEnqueueError> {
        self.queue.enqueue(bio)
    }

    fn metadata(&self) -> BlockDeviceMeta {
        BlockDeviceMeta {
            max_nr_segments_per_bio: self.queue.max_nr_segments_per_bio(),
            nr_sectors: self.device.config_manager.capacity_sectors(),
        }
    }

    fn id_allocator(&self) -> &'static DeviceIdAllocator {
        VIRTIO_BLOCK_ID_ALLOCATOR.get().unwrap()
    }

    fn set_partitions(&self, infos: Vec<Option<PartitionInfo>>) {
        let mut new_partitions = Vec::new();
        for (index, info_opt) in infos.iter().enumerate() {
            let Some(info) = info_opt else {
                continue;
            };

            let index = index as u32 + 1;
            let id = if index < LEGACY_PARTITION_LIMIT {
                self.id_allocator()
                    .allocate(self.id.minor() + index)
                    .unwrap()
            } else {
                alloc_extended_device_id(0)
            };
            let name = format!("{}{}", self.name(), index);
            let device = self.weak_self().clone();

            let partition = PartitionNode::new(index, id, name, device, *info);
            new_partitions.push(partition);
        }

        let mut partitions = self.partitions.lock();
        if let Some(old_partitions) = partitions.as_ref() {
            old_partitions.iter().for_each(|p| {
                let _ = self.remove_child(p.name());
            });
        };

        new_partitions.iter().for_each(|p| {
            let _ = self.add_child(p.clone());
        });

        *partitions = Some(new_partitions);
    }

    fn partitions(&self) -> Option<Vec<Arc<dyn aster_block::BlockDevice>>> {
        let partitions = self.partitions.lock();
        let devices = partitions
            .as_ref()?
            .iter()
            .map(|p| p.clone() as Arc<dyn aster_block::BlockDevice>)
            .collect();
        Some(devices)
    }
}

impl Drop for BlockDevice {
    fn drop(&mut self) {
        use aster_block::BlockDevice;

        self.id_allocator().release(self.id.minor());
    }
}

#[derive(Debug)]
struct DeviceInner {
    config_manager: ConfigManager<VirtioBlockConfig>,
    features: VirtioBlockFeature,
    queue: SpinLock<VirtQueue>,
    transport: SpinLock<Box<dyn VirtioTransport>>,
    block_requests: Arc<DmaStream>,
    block_responses: Arc<DmaStream>,
    id_allocator: SpinLock<IdAlloc>,
    submitted_requests: SpinLock<BTreeMap<u16, SubmittedRequest>>,
}

impl DeviceInner {
    const QUEUE_SIZE: u16 = 64;

    /// Creates and inits the device.
    pub fn init(mut transport: Box<dyn VirtioTransport>) -> Result<Arc<Self>, VirtioDeviceError> {
        let config_manager = VirtioBlockConfig::new_manager(transport.as_ref());
        debug!("virio_blk_config = {:?}", config_manager.read_config());
        assert_eq!(
            config_manager.block_size(),
            VirtioBlockConfig::sector_size(),
            "currently not support customized device logical block size"
        );
        let num_queues = transport.num_queues();
        if num_queues != 1 {
            // FIXME: support Multi-Queue Block IO Queueing Mechanism
            // (`BlkFeatures::MQ`) to accelerate multi-processor requests for
            // block devices. When SMP is enabled on x86, the feature is on.
            // We should also consider negotiating the feature in the future.
            // return Err(VirtioDeviceError::QueuesAmountDoNotMatch(num_queues, 1));
            log::warn!(
                "Not supporting Multi-Queue Block IO Queueing Mechanism, only using the first queue"
            );
        }
        let features = VirtioBlockFeature::new(transport.as_ref());
        let queue = VirtQueue::new(0, Self::QUEUE_SIZE, transport.as_mut())
            .expect("create virtqueue failed");
        let block_requests = {
            let segment = FrameAllocOptions::new().alloc_segment(1).unwrap();
            Arc::new(DmaStream::map(segment.into(), DmaDirection::Bidirectional, false).unwrap())
        };
        assert!(Self::QUEUE_SIZE as usize * REQ_SIZE <= block_requests.size());
        let block_responses = {
            let segment = FrameAllocOptions::new().alloc_segment(1).unwrap();
            Arc::new(DmaStream::map(segment.into(), DmaDirection::Bidirectional, false).unwrap())
        };
        assert!(Self::QUEUE_SIZE as usize * RESP_SIZE <= block_responses.size());

        let device = Arc::new(Self {
            config_manager,
            features,
            queue: SpinLock::new(queue),
            transport: SpinLock::new(transport),
            block_requests,
            block_responses,
            id_allocator: SpinLock::new(IdAlloc::with_capacity(Self::QUEUE_SIZE as usize)),
            submitted_requests: SpinLock::new(BTreeMap::new()),
        });

        let cloned_device = device.clone();
        let handle_irq = move |_: &TrapFrame| {
            cloned_device.handle_irq();
        };

        let cloned_device = device.clone();
        let handle_config_change = move |_: &TrapFrame| {
            cloned_device.handle_config_change();
        };

        {
            let mut transport = device.transport.lock();
            transport
                .register_cfg_callback(Box::new(handle_config_change))
                .unwrap();
            transport
                .register_queue_callback(0, Box::new(handle_irq), false)
                .unwrap();
            transport.finish_init();
        }

        Ok(device)
    }

    /// Handles the irq issued from the device
    fn handle_irq(&self) {
        info!("Virtio block device handle irq");
        // When we enter the IRQs handling function,
        // IRQs have already been disabled,
        // so there is no need to call `disable_irq`.
        loop {
            // Pops the complete request
            let complete_request = {
                let mut queue = self.queue.lock();
                let Ok((token, _)) = queue.pop_used() else {
                    return;
                };
                self.submitted_requests.lock().remove(&token).unwrap()
            };

            // Handles the response
            let id = complete_request.id as usize;
            let resp_slice =
                Slice::new(&self.block_responses, id * RESP_SIZE..(id + 1) * RESP_SIZE);
            resp_slice.sync().unwrap();
            let resp: BlockResp = resp_slice.read_val(0).unwrap();
            self.id_allocator.lock().free(id);
            match RespStatus::try_from(resp.status).unwrap() {
                RespStatus::Ok => {}
                // FIXME: Return an error instead of triggering a kernel panic
                _ => panic!("io error in block device"),
            };

            // Synchronize DMA mapping if read from the device
            if let BioType::Read = complete_request.bio_request.type_() {
                complete_request
                    .bio_request
                    .bios()
                    .flat_map(|bio| {
                        bio.segments()
                            .iter()
                            .map(|segment| segment.inner_dma_slice())
                    })
                    .for_each(|dma_slice| dma_slice.sync().unwrap());
            }

            // Completes the bio request
            complete_request.bio_request.bios().for_each(|bio| {
                bio.complete(BioStatus::Complete);
            });
        }
    }

    fn handle_config_change(&self) {
        info!("Virtio block device config space change");
    }

    /// Reads data from the device, this function is non-blocking.
    fn read(&self, bio_request: BioRequest) {
        let id = self.id_allocator.disable_irq().lock().alloc().unwrap();
        let req_slice = {
            let req_slice = Slice::new(
                self.block_requests.clone(),
                id * REQ_SIZE..(id + 1) * REQ_SIZE,
            );
            let req = BlockReq {
                type_: ReqType::In as _,
                reserved: 0,
                sector: bio_request.sid_range().start.to_raw(),
            };
            req_slice.write_val(0, &req).unwrap();
            req_slice.sync().unwrap();
            req_slice
        };

        let resp_slice = {
            let resp_slice = Slice::new(
                self.block_responses.clone(),
                id * RESP_SIZE..(id + 1) * RESP_SIZE,
            );
            resp_slice.write_val(0, &BlockResp::default()).unwrap();
            resp_slice
        };

        let outputs = {
            let mut outputs: Vec<&Slice<_>> = Vec::with_capacity(bio_request.num_segments() + 1);
            let dma_slices_iter = bio_request.bios().flat_map(|bio| {
                bio.segments()
                    .iter()
                    .map(|segment| segment.inner_dma_slice())
            });
            outputs.extend(dma_slices_iter);
            outputs.push(&resp_slice);
            outputs
        };

        let num_used_descs = outputs.len() + 1;
        // FIXME: Split the request if it is too big
        if num_used_descs > Self::QUEUE_SIZE as usize {
            panic!("The request size surpasses the queue size");
        }

        loop {
            let mut queue = self.queue.disable_irq().lock();
            if num_used_descs > queue.available_desc() {
                continue;
            }
            let token = queue
                .add_dma_buf(&[&req_slice], outputs.as_slice())
                .expect("add queue failed");
            if queue.should_notify() {
                queue.notify();
            }

            // Records the submitted request
            let submitted_request = SubmittedRequest::new(id as u16, bio_request);
            self.submitted_requests
                .disable_irq()
                .lock()
                .insert(token, submitted_request);
            return;
        }
    }

    /// Writes data to the device, this function is non-blocking.
    fn write(&self, bio_request: BioRequest) {
        let id = self.id_allocator.disable_irq().lock().alloc().unwrap();
        let req_slice = {
            let req_slice = Slice::new(
                self.block_requests.clone(),
                id * REQ_SIZE..(id + 1) * REQ_SIZE,
            );
            let req = BlockReq {
                type_: ReqType::Out as _,
                reserved: 0,
                sector: bio_request.sid_range().start.to_raw(),
            };
            req_slice.write_val(0, &req).unwrap();
            req_slice.sync().unwrap();
            req_slice
        };

        let resp_slice = {
            let resp_slice = Slice::new(
                self.block_responses.clone(),
                id * RESP_SIZE..(id + 1) * RESP_SIZE,
            );
            resp_slice.write_val(0, &BlockResp::default()).unwrap();
            resp_slice
        };

        let inputs = {
            let mut inputs: Vec<&Slice<_>> = Vec::with_capacity(bio_request.num_segments() + 1);
            inputs.push(&req_slice);
            let dma_slices_iter = bio_request.bios().flat_map(|bio| {
                bio.segments()
                    .iter()
                    .map(|segment| segment.inner_dma_slice())
            });
            inputs.extend(dma_slices_iter);
            inputs
        };

        let num_used_descs = inputs.len() + 1;
        // FIXME: Split the request if it is too big
        if num_used_descs > Self::QUEUE_SIZE as usize {
            panic!("The request size surpasses the queue size");
        }
        loop {
            let mut queue = self.queue.disable_irq().lock();
            if num_used_descs > queue.available_desc() {
                continue;
            }
            let token = queue
                .add_dma_buf(inputs.as_slice(), &[&resp_slice])
                .expect("add queue failed");
            if queue.should_notify() {
                queue.notify();
            }

            // Records the submitted request
            let submitted_request = SubmittedRequest::new(id as u16, bio_request);
            self.submitted_requests
                .disable_irq()
                .lock()
                .insert(token, submitted_request);
            return;
        }
    }

    /// Flushes any cached data from the guest to the persistent storage on the host.
    /// This will be ignored if the device doesn't support the `VIRTIO_BLK_F_FLUSH` feature.
    fn flush(&self, bio_request: BioRequest) {
        if self.features.support_flush {
            bio_request.bios().for_each(|bio| {
                bio.complete(BioStatus::Complete);
            });
            return;
        }

        let id = self.id_allocator.disable_irq().lock().alloc().unwrap();
        let req_slice = {
            let req_slice = Slice::new(&self.block_requests, id * REQ_SIZE..(id + 1) * REQ_SIZE);
            let req = BlockReq {
                type_: ReqType::Flush as _,
                reserved: 0,
                sector: bio_request.sid_range().start.to_raw(),
            };
            req_slice.write_val(0, &req).unwrap();
            req_slice.sync().unwrap();
            req_slice
        };

        let resp_slice = {
            let resp_slice =
                Slice::new(&self.block_responses, id * RESP_SIZE..(id + 1) * RESP_SIZE);
            resp_slice.write_val(0, &BlockResp::default()).unwrap();
            resp_slice
        };

        let num_used_descs = 1;
        loop {
            let mut queue = self.queue.disable_irq().lock();
            if num_used_descs > queue.available_desc() {
                continue;
            }
            let token = queue
                .add_dma_buf(&[&req_slice], &[&resp_slice])
                .expect("add queue failed");
            if queue.should_notify() {
                queue.notify();
            }

            // Records the submitted request
            let submitted_request = SubmittedRequest::new(id as u16, bio_request);
            self.submitted_requests
                .disable_irq()
                .lock()
                .insert(token, submitted_request);
            return;
        }
    }
}

/// A submitted bio request for callback.
#[derive(Debug)]
struct SubmittedRequest {
    id: u16,
    bio_request: BioRequest,
}

impl SubmittedRequest {
    pub fn new(id: u16, bio_request: BioRequest) -> Self {
        Self { id, bio_request }
    }
}

/// VirtIOBlock request.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
struct BlockReq {
    pub type_: u32,
    pub reserved: u32,
    pub sector: u64,
}

const REQ_SIZE: usize = size_of::<BlockReq>();

/// Response of a VirtIOBlock request.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
struct BlockResp {
    pub status: u8,
}

const RESP_SIZE: usize = size_of::<BlockResp>();

impl Default for BlockResp {
    fn default() -> Self {
        Self {
            status: RespStatus::_NotReady as _,
        }
    }
}
