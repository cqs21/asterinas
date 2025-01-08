use alloc::{
    boxed::Box,
    string::String,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    fmt::Debug,
    sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};

use log::debug;
use ostd_pod::Pod;
use xhci::{
    context::{EndpointState, EndpointType, SlotState},
    ring::trb::{
        command, event,
        transfer::{self, TransferType},
    },
    Registers,
};

use super::{
    class::UsbDevice,
    descriptor::{Descriptor, DescriptorIter},
    device_contexts::InputContext,
    requests::{
        DeviceFeatureSelector, Direction, EndpointFeatureSelector, GetConfiguration, GetDescriptor,
        GetStatus, InterfaceFeatureSelector, Recipient, Request, SetConfiguration,
    },
    transfer_ring::TransferRing,
    CommandRing, ConfigurationDescriptor, DeviceDescriptor, DoorbellReason, DEFAULT_RING_SIZE,
};
use crate::{
    bus::usb::{
        descriptor::{BosDescriptor, DescriptorWrapper, StringDescriptor},
        EndpointDescriptor, InterfaceAssociationDescriptor, InterfaceDescriptor,
    },
    io_mem::IoMem,
    mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions, VmIo, PAGE_SIZE},
    sync::SpinLock,
};

pub struct XhciSlot {
    // Slot ID (Valid values are 1, 2, 3, ... MaxSlots).
    slot_id: AtomicU8,
    // Port ID (Valid values are 1, 2, 3, ... MaxPorts).
    port_id: u8,
    port_speed: u8,
    regs: Weak<SpinLock<Registers<IoMem>>>,
    command_ring: Weak<CommandRing>,
    input_context: SpinLock<InputContext>,
    endpoint_rings: SpinLock<Vec<Option<TransferRing>>>,
    device_descriptor: SpinLock<DeviceDescriptor>,
    interface_descriptors: SpinLock<Vec<InterfaceDescriptor>>,
    endpoint_descriptors: SpinLock<Vec<EndpointDescriptor>>,
    dma_buffer: DmaCoherent,
    buffer_length: AtomicUsize,
    fs_evaluated: AtomicBool,
    completion_callbacks: SpinLock<Vec<Box<CommandCompletionCallback>>>,
    event_callbacks: SpinLock<Vec<Box<TransferEventCallback>>>,
}

pub type CommandCompletionCallback = dyn Fn(&event::CommandCompletion) + Send + Sync;

pub type TransferEventCallback = dyn Fn(&event::TransferEvent) + Send + Sync;

const DEFAULT_ENDPOINT_SIZE: usize = 31;

impl XhciSlot {
    pub(super) fn init(
        port_id: u8,
        port_speed: u8,
        is_64bytes_context: bool,
        regs: Weak<SpinLock<Registers<IoMem>>>,
        command_ring: Weak<CommandRing>,
    ) -> Self {
        let mut input_context = InputContext::new(is_64bytes_context);
        // Initialize the Input Control Context by setting the A0 and A1 flags to ‘1’.
        // These flags indicate that the Slot Context and the Endpoint 0 Context of
        // the Input Context are affected by the next AddressDevice command.
        {
            let ctrl = input_context.handle_mut().control_mut();
            ctrl.set_add_context_flag(0);
            ctrl.set_add_context_flag(1);
        }
        // Initialize the Input Slot Context.
        {
            let slot = input_context.handle_mut().device_mut().slot_mut();
            slot.set_root_hub_port_number(port_id);
            // FIXME: route string is topology defined. Refer to section 8.9 in the USB3 spec.
            slot.set_route_string(0);
            slot.set_context_entries(1);
        }

        // Allocate the Transfer Ring for the Default Control Endpoint.
        let ctrl_ring = TransferRing::with_capacity(DEFAULT_RING_SIZE);
        // Initialize the Input default control Endpoint Context 0.
        {
            let ep0 = input_context.handle_mut().device_mut().endpoint_mut(1);
            ep0.set_endpoint_type(EndpointType::Control);
            // For LS, HS, and SS devices; 8, 64, and 512 bytes, respectively, are the
            // only packet sizes allowed for the Default Control Endpoint.
            // For FS device, system software should initially read the first 8 bytes of
            // the USB Device Descriptor to retrieve the value of the bMaxPacketSize0 field.
            ep0.set_max_packet_size(Self::speed_id_mapping(port_speed));
            ep0.set_max_burst_size(0);
            ep0.set_tr_dequeue_pointer(ctrl_ring.base() as u64);
            ep0.set_dequeue_cycle_state();
            ep0.set_interval(0);
            ep0.set_max_primary_streams(0);
            ep0.set_mult(0);
            ep0.set_error_count(3);
        }

        // Allocate a buffer, could be used for GetDescriptor or SetDescriptor request.
        let seg = FrameAllocOptions::new().alloc_segment(1).unwrap();
        let dma_buffer = DmaCoherent::map(seg.into(), true).unwrap();

        let mut endpoint_rings = Vec::with_capacity(DEFAULT_ENDPOINT_SIZE);
        endpoint_rings.push(Some(ctrl_ring));
        (1..DEFAULT_ENDPOINT_SIZE).for_each(|_| endpoint_rings.push(None));

        Self {
            // Slot ID will be initialized in handle_command_completion of EnableSlot Command.
            slot_id: AtomicU8::new(0),
            port_id,
            port_speed,
            regs,
            command_ring,
            input_context: SpinLock::new(input_context),
            endpoint_rings: SpinLock::new(endpoint_rings),
            device_descriptor: SpinLock::new(DeviceDescriptor::new_zeroed()),
            interface_descriptors: SpinLock::new(Vec::new()),
            endpoint_descriptors: SpinLock::new(Vec::new()),
            dma_buffer,
            buffer_length: AtomicUsize::new(0),
            fs_evaluated: AtomicBool::new(false),
            completion_callbacks: SpinLock::new(Vec::new()),
            event_callbacks: SpinLock::new(Vec::new()),
        }
    }

    pub(super) fn slot_id(&self) -> u8 {
        self.slot_id.load(Ordering::Relaxed)
    }

    pub(super) fn set_slot_id(&self, slot_id: u8) {
        self.slot_id.store(slot_id, Ordering::Relaxed);
    }

    pub(super) fn port_id(&self) -> u8 {
        self.port_id
    }

    pub(super) fn input_context_base(&self) -> usize {
        self.input_context.lock().base()
    }

    pub(super) fn output_device_context_base(&self) -> usize {
        self.input_context.lock().output_device_context_base()
    }

    pub(super) fn handle_command_completion(&self, cc: event::CommandCompletion) {
        match self.slot_state() {
            SlotState::DisabledEnabled => {
                let set_bsr = if self.is_full_speed() { true } else { false };
                self.address_device(set_bsr);
            }
            SlotState::Default => {
                if self.is_full_speed() && self.fs_evaluated() == false {
                    self.get_descriptor_for_fs();
                    return;
                }
                if self.is_full_speed() && self.fs_evaluated() == true {
                    self.address_device(false);
                    return;
                }
            }
            SlotState::Addressed => {
                // FIXME: system software may read the complete USB Device Descriptor
                // and possibly the Configuration Descriptors so that it can hand the
                // device off to the appropriate Class Driver, then issue a Configure
                // Endpoint Command.
                //
                // TODO: handle other state transformation, e.g. Configured -> Addressed.
                self.configure_endpoint();
            }
            SlotState::Configured => {
                if self.device_descriptor.lock().length == 0 {
                    self.get_device_descriptor();
                }
                debug!("TODO: handle configured completion event");
            }
        };

        let completion_callbacks = self.completion_callbacks.lock();
        for callback in completion_callbacks.iter() {
            callback(&cc);
        }
    }

    pub(super) fn handle_transfer_event(&self, te: event::TransferEvent) {
        let endpoint_id = te.endpoint_id() as usize;
        let trb_pointer = te.trb_pointer() as usize;
        let mut endpoint_rings = self.endpoint_rings.lock();
        let Some(ring) = endpoint_rings[endpoint_id - 1].as_mut() else {
            return;
        };
        if ring.should_skip(trb_pointer) {
            return;
        }
        // Update Transfer Ring Dequeue Pointer.
        ring.set_dequeue_pointer(trb_pointer);
        drop(endpoint_rings);

        match self.slot_state() {
            SlotState::DisabledEnabled => {
                unreachable!("slot should not be in DisabledEnabled state")
            }
            SlotState::Default => {
                if self.is_full_speed() && endpoint_id == 1 && self.fs_evaluated() == false {
                    self.evaluate_context_for_fs();
                }
            }
            SlotState::Addressed => debug!("TODO: handle addressed transfer event"),
            SlotState::Configured => {
                let descriptors: Vec<_> = DescriptorIter::new(self.dma_buffer()).collect();
                descriptors.into_iter().for_each(|wrapper| match wrapper {
                    DescriptorWrapper::Device(d) => *self.device_descriptor.lock() = d,
                    DescriptorWrapper::Interface(i) => self.interface_descriptors.lock().push(i),
                    DescriptorWrapper::Endpoint(e) => self.endpoint_descriptors.lock().push(e),
                    _ => (),
                });
                if self.interface_descriptors.lock().is_empty() {
                    self.get_configuration_descriptor();
                }

                debug!("{:?}", self.device_descriptor.lock());
                debug!("{:?}", self.interface_descriptors.lock());
                debug!("{:?}", self.endpoint_descriptors.lock());
            }
        };

        let event_callbacks = self.event_callbacks.lock();
        for callback in event_callbacks.iter() {
            callback(&te);
        }
    }

    fn buffer_length(&self) -> usize {
        self.buffer_length.load(Ordering::Relaxed)
    }

    fn set_buffer_length(&self, length: usize) {
        self.buffer_length.store(length, Ordering::Relaxed);
    }

    fn fs_evaluated(&self) -> bool {
        self.fs_evaluated.load(Ordering::Relaxed)
    }

    fn set_fs_evaluated(&self, evaluated: bool) {
        self.fs_evaluated.store(evaluated, Ordering::Relaxed);
    }

    fn speed_id_mapping(port_speed: u8) -> u16 {
        match port_speed {
            1 => 64,  // Full-speed
            2 => 8,   // Low-speed
            3 => 64,  // High-speed
            4 => 512, // SuperSpeed Gen1 x1
            5 => 512, // SuperSpeed Gen2 x1
            6 => 512, // SuperSpeed Gen1 x2
            7 => 512, // SuperSpeed Gen2 x2
            _ => panic!("unrecognized xhci port speed id"),
        }
    }

    fn is_full_speed(&self) -> bool {
        self.port_speed == 1
    }

    fn dma_buffer(&self) -> &[u8] {
        let len = self.buffer_length();
        debug_assert!(len <= PAGE_SIZE);
        let va = paddr_to_vaddr(self.dma_buffer.start_paddr());
        self.set_buffer_length(0);
        unsafe { core::slice::from_raw_parts(va as *const u8, len) }
    }

    fn ring_doorbell_at(&self, idx: usize, target: u8, stream_id: u16) {
        let Some(regs) = self.regs.upgrade() else {
            return;
        };
        regs.lock().doorbell.update_volatile_at(idx, |db| {
            db.set_doorbell_target(target);
            db.set_doorbell_stream_id(stream_id);
        });
    }

    fn add_command(&self, allowed: command::Allowed) {
        let Some(ring) = self.command_ring.upgrade() else {
            return;
        };

        ring.enqueue(allowed).unwrap();
        self.ring_doorbell_at(0, 0, 0);
    }

    fn address_device(&self, set_bsr: bool) {
        // Issue an Address Device Command for the Device Slot.
        let mut address = command::AddressDevice::default();
        address.set_slot_id(self.slot_id());
        address.set_input_context_pointer(self.input_context_base() as u64);
        // For some legacy USB devices it may be necessary to communicate with the
        // device when it is in the Default state, before transitioning it to the
        // Address state. To accomplish this system software shall issue an AddressDevice
        // Command with the BSR flag set to ‘1’.
        if set_bsr {
            address.set_block_set_address_request();
        } else {
            address.clear_block_set_address_request();
        }
        let allowed = command::Allowed::AddressDevice(address);
        self.add_command(allowed);
    }

    fn configure_endpoint(&self) {
        let mut input_context = self.input_context.lock();
        // The Add Context flag A0 shall be set to ‘1’, A1 shall be cleared to ‘0’.
        let input_ctrl = input_context.handle_mut().control_mut();
        input_ctrl.set_add_context_flag(0);
        input_ctrl.clear_add_context_flag(1);

        // Configure the Device Slot using a Configure Endpoint Command.
        let mut configure = command::ConfigureEndpoint::default();
        configure.set_slot_id(self.slot_id());
        configure.clear_deconfigure();
        configure.set_input_context_pointer(input_context.base() as u64);
        let allowed = command::Allowed::ConfigureEndpoint(configure);
        self.add_command(allowed);
    }

    fn evaluate_context_for_fs(&self) {
        let device = DescriptorIter::new(self.dma_buffer())
            .find_map(|des| match des {
                DescriptorWrapper::Device(device) => Some(device),
                _ => None,
            })
            .unwrap();

        let mut input_context = self.input_context.lock();
        let ep0 = input_context.handle_mut().device_mut().endpoint_mut(1);
        ep0.set_max_packet_size(device.max_packet_size as u16);

        let mut evaluate = command::EvaluateContext::default();
        evaluate.set_slot_id(self.slot_id());
        evaluate.set_input_context_pointer(input_context.base() as u64);
        let allowed = command::Allowed::EvaluateContext(evaluate);

        self.add_command(allowed);
        self.set_fs_evaluated(true);
    }

    /// For FS devices, system software should initially read the first 8 bytes of
    /// the USB Device Descriptor to retrieve the value of the MaxPacketSize filed
    /// and determine the actual Max Packet Size for the Default Control Endpoint.
    fn get_descriptor_for_fs(&self) {
        let get_device = GetDescriptor::new(Descriptor::Device, 0, 0, &self.dma_buffer, 8);
        let request = Request::GetDescriptor(get_device);
        self.set_buffer_length(8);
        self.send_device_request(request);
    }

    fn get_device_descriptor(&self) {
        let length = size_of::<DeviceDescriptor>();
        let get_descriptor =
            GetDescriptor::new(Descriptor::Device, 0, 0, &self.dma_buffer, length as u16);
        let request = Request::GetDescriptor(get_descriptor);
        self.set_buffer_length(length);
        self.send_device_request(request);
    }

    fn get_configuration_descriptor(&self) {
        let get_descriptor =
            GetDescriptor::new(Descriptor::Configuration, 0, 0, &self.dma_buffer, 1024);
        let request = Request::GetDescriptor(get_descriptor);
        self.set_buffer_length(1024);
        self.send_device_request(request);
    }
}

impl Debug for XhciSlot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("XhciSlot")
            .field("slot_id", &self.slot_id())
            .field("port_id", &self.port_id())
            .field("port_speed", &self.port_speed)
            .field("device", &self.device_descriptor.lock())
            .field("interfaces", &self.interface_descriptors.lock())
            .field("endpoints", &self.endpoint_descriptors.lock())
            .finish()
    }
}

impl UsbDevice for XhciSlot {
    fn slot_id(&self) -> u8 {
        self.slot_id()
    }

    fn slot_state(&self) -> SlotState {
        self.input_context
            .lock()
            .output_device()
            .slot()
            .slot_state()
    }

    fn device(&self) -> DeviceDescriptor {
        self.device_descriptor.lock().clone()
    }

    fn interfaces(&self) -> Vec<InterfaceDescriptor> {
        self.interface_descriptors.lock().clone()
    }

    fn endpoints(&self) -> Vec<EndpointDescriptor> {
        self.endpoint_descriptors.lock().clone()
    }

    fn endpoint_state(&self, endpoint: &EndpointDescriptor) -> EndpointState {
        let dci = endpoint.device_context_index() as usize;
        self.input_context
            .lock()
            .output_device()
            .endpoint(dci)
            .endpoint_state()
    }

    fn enable_endpoint(&self, endpoint: &EndpointDescriptor) {
        let direction = endpoint.direction();
        let dci = endpoint.device_context_index() as usize;
        let ep_type = match endpoint.attributes & 0x3 {
            0 => EndpointType::Control,
            1 if direction == Direction::Out => EndpointType::IsochOut,
            1 if direction == Direction::In => EndpointType::IsochIn,
            2 if direction == Direction::Out => EndpointType::BulkOut,
            2 if direction == Direction::In => EndpointType::BulkIn,
            3 if direction == Direction::Out => EndpointType::InterruptOut,
            3 if direction == Direction::In => EndpointType::InterruptIn,
            _ => EndpointType::NotValid,
        };
        let max_packet_size = endpoint.max_packet_size & 0x7FF;
        // FIXME: refer to SuperSpeed Endpoint Companion Descriptor:bMaxBurst.
        let max_burst_size = ((endpoint.max_packet_size & 0x1800) >> 11) as u8;
        // FIXME: refer to SuperSpeed Endpoint Companion Descriptor:bmAttributes:Mult field.
        // Always ‘0’ for Interrupt endpoints.
        let mult = 0;
        // Retries are not performed for Isoch endpoints when encounters an error.
        let error_count = if endpoint.attributes & 0x3 == 1 { 0 } else { 3 };
        let interval = endpoint.interval;
        // Allocate the Transfer Ring.
        let ring = TransferRing::with_capacity(DEFAULT_RING_SIZE);
        // Initialize the Endpoint Context in the Input Context.
        {
            let mut input_context = self.input_context.lock();
            let device = input_context.handle_mut().device_mut();
            device.slot_mut().set_context_entries(dci as u8 + 1);

            let ep = device.endpoint_mut(dci);
            ep.set_endpoint_type(ep_type);
            ep.set_max_packet_size(max_packet_size);
            ep.set_max_burst_size(max_burst_size);
            ep.set_mult(mult);
            ep.set_error_count(error_count);
            ep.set_interval(interval);
            ep.set_tr_dequeue_pointer(ring.base() as u64);
            ep.set_dequeue_cycle_state();
        }

        debug_assert!(dci <= DEFAULT_ENDPOINT_SIZE);
        let mut endpoint_rings = self.endpoint_rings.lock();
        let old = endpoint_rings[dci - 1].replace(ring);
        debug_assert!(old.is_none());

        // The Configure Endpoint Command (Add (A)= ‘1’and Drop (D) = ‘0’)
        // shall transition an endpoint, except the Default Control Endpoint,
        // from the Disabled to the Running state.
        let mut input_context = self.input_context.lock();
        let input_ctrl = input_context.handle_mut().control_mut();
        input_ctrl.set_add_context_flag(dci);
        input_ctrl.clear_drop_context_flag(dci);

        // Issue a Configure Endpoint Command.
        let mut configure = command::ConfigureEndpoint::default();
        configure.set_slot_id(self.slot_id());
        configure.clear_deconfigure();
        configure.set_input_context_pointer(input_context.base() as u64);
        let allowed = command::Allowed::ConfigureEndpoint(configure);
        self.add_command(allowed);
    }

    fn send_device_request(&self, request: Request) {
        let mut endpoint_rings = self.endpoint_rings.lock();
        let ctrl_ring = endpoint_rings[0].as_mut().unwrap();
        request.into_iter().for_each(|trb| {
            ctrl_ring.enqueue(trb).unwrap();
        });

        // DB Target = Control EP 0 Enqueue Pointer Update.
        self.ring_doorbell_at(self.slot_id() as usize, 1, 0);
    }

    fn send_endpoint_request(&self, endpoint: &EndpointDescriptor, allowed: transfer::Allowed) {
        let dci = endpoint.device_context_index();
        let mut endpoint_rings = self.endpoint_rings.lock();
        let Some(ring) = endpoint_rings[(dci - 1) as usize].as_mut() else {
            return;
        };

        ring.enqueue(allowed).unwrap();
        self.ring_doorbell_at(self.slot_id() as usize, dci, 0);
    }

    fn register_completion_callback(&self, callback: Box<CommandCompletionCallback>) {
        self.completion_callbacks.lock().push(callback);
    }

    fn register_event_callback(&self, callback: Box<TransferEventCallback>) {
        self.event_callbacks.lock().push(callback);
    }
}
