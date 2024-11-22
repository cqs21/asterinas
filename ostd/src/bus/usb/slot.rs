use alloc::vec::Vec;

use xhci::{
    context::{EndpointType, SlotState},
    ring::trb::{
        command, event,
        transfer::{self, TransferType},
    },
};

use super::{
    device_contexts::InputContext, transfer_ring::TransferRing, ConfigurationDescriptor,
    DeviceDescriptor, DoorbellReason, DEFAULT_RING_SIZE,
};
use crate::mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions, PAGE_SIZE};

#[derive(Debug)]
pub struct XhciSlot {
    // Slot ID (Valid values are 1, 2, 3, ... MaxSlots).
    slot_id: u8,
    // Port ID (Valid values are 1, 2, 3, ... MaxPorts).
    port_id: u8,
    port_speed: u8,
    input_context: InputContext,
    endpoint_rings: Vec<Option<TransferRing>>,
    pending_commands: Option<Vec<command::Allowed>>,
    pending_doorbell: Option<DoorbellReason>,
    extra_buffer: DmaCoherent,
    fs_evaluated: bool,
}

const DEFAULT_ENDPOINT_SIZE: usize = 31;

impl XhciSlot {
    pub fn init(port_id: u8, port_speed: u8, is_64bytes_context: bool) -> Self {
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
        let mut ctrl_ring = TransferRing::with_capacity(DEFAULT_RING_SIZE);
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

        // Allocate extra buffer, may be used for fs device.
        let seg = FrameAllocOptions::new().alloc_segment(1).unwrap();
        let extra_buffer = DmaCoherent::map(seg.into(), true).unwrap();

        let mut endpoint_rings = Vec::with_capacity(DEFAULT_ENDPOINT_SIZE);
        endpoint_rings.push(Some(ctrl_ring));
        (1..DEFAULT_ENDPOINT_SIZE).for_each(|_| endpoint_rings.push(None));

        Self {
            // Slot ID will be initialized in handle_command_completion of EnableSlot Command.
            slot_id: 0,
            port_id,
            port_speed,
            input_context,
            endpoint_rings,
            pending_commands: None,
            pending_doorbell: None,
            extra_buffer,
            fs_evaluated: false,
        }
    }

    fn ctrl_ring(&mut self) -> &mut TransferRing {
        self.endpoint_rings[0].as_mut().unwrap()
    }

    pub fn slot_id(&self) -> u8 {
        self.slot_id
    }

    pub fn set_slot_id(&mut self, slot_id: u8) {
        self.slot_id = slot_id;
    }

    pub fn port_id(&self) -> u8 {
        self.port_id
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

    fn extra_data<T>(&mut self) -> &mut T {
        debug_assert!(size_of::<T>() < PAGE_SIZE);
        let va = paddr_to_vaddr(self.extra_buffer.start_paddr());
        unsafe { &mut *(va as *mut T) }
    }

    pub fn input_context_base(&self) -> usize {
        self.input_context.base()
    }

    pub fn output_device_context_base(&self) -> usize {
        self.input_context.output_device_context_base()
    }

    pub fn pending_commands(&mut self) -> Option<Vec<command::Allowed>> {
        self.pending_commands.take()
    }

    pub fn pending_doorbell(&mut self) -> Option<DoorbellReason> {
        self.pending_doorbell.take()
    }

    pub fn handle_command_completion(&mut self) {
        let slot_state = self.input_context.output_device().slot().slot_state();
        match slot_state {
            SlotState::DisabledEnabled => {
                let set_bsr = if self.is_full_speed() { true } else { false };
                self.address_device(set_bsr);
            }
            SlotState::Default => {
                if self.is_full_speed() && self.fs_evaluated == false {
                    self.get_descriptor_for_fs();
                    return;
                }
                if self.is_full_speed() && self.fs_evaluated == true {
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
                self.get_device_descriptor();
                crate::early_println!("TODO: handle configured completion event")
            }
        };
    }

    fn address_device(&mut self, set_bsr: bool) {
        // Issue an Address Device Command for the Device Slot.
        let mut address = command::AddressDevice::default();
        address.set_slot_id(self.slot_id);
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
        self.pending_commands.get_or_insert_default().push(allowed);
    }

    fn configure_endpoint(&mut self) {
        // The Add Context flag A0 shall be set to ‘1’, A1 shall be cleared to ‘0’.
        let input_ctrl = self.input_context.handle_mut().control_mut();
        input_ctrl.set_add_context_flag(0);
        input_ctrl.clear_add_context_flag(1);

        // Configure the Device Slot using a Configure Endpoint Command.
        let mut configure = command::ConfigureEndpoint::default();
        configure.set_slot_id(self.slot_id);
        configure.clear_deconfigure();
        configure.set_input_context_pointer(self.input_context_base() as u64);
        let allowed = command::Allowed::ConfigureEndpoint(configure);
        self.pending_commands.get_or_insert_default().push(allowed);
    }

    /// For FS devices, system software should initially read the first 8 bytes of
    /// the USB Device Descriptor to retrieve the value of the bMaxPacketSize0
    /// field and determine the actual Max Packet Size for the Default Control
    /// Endpoint.
    fn get_descriptor_for_fs(&mut self) {
        // Initialize the Setup Stage TD.
        let mut setup = transfer::SetupStage::default();
        setup.set_transfer_type(TransferType::In);
        setup.clear_interrupt_on_completion();
        setup.set_request_type(0x80); // Dir = Device-to-Host, Type = Standard, Recipient = Device
        setup.set_request(6); // GET_DESCRIPTOR
        setup.set_value(0x0100); // Descriptor Index = 0, Descriptor Type = 1
        setup.set_index(0);
        setup.set_length(8);
        let allowed = transfer::Allowed::SetupStage(setup);
        self.ctrl_ring().enqueue(allowed).unwrap();

        // Initialize the Data Stage TD.
        let mut data = transfer::DataStage::default();
        data.set_direction(transfer::Direction::In);
        data.set_trb_transfer_length(8);
        data.clear_chain_bit();
        data.clear_interrupt_on_completion();
        data.clear_immediate_data();
        data.set_data_buffer_pointer(self.extra_buffer.start_paddr() as u64);
        let allowed = transfer::Allowed::DataStage(data);
        self.ctrl_ring().enqueue(allowed).unwrap();

        // Initialize the Status Stage TD.
        let mut status = transfer::StatusStage::default();
        status.clear_direction();
        status.clear_chain_bit();
        status.set_interrupt_on_completion();
        let allowed = transfer::Allowed::StatusStage(status);
        self.ctrl_ring().enqueue(allowed).unwrap();

        self.pending_doorbell.replace((1, 0)); // DB Target = Control EP 0 Enqueue Pointer Update.
    }

    pub fn handle_transfer_event(&mut self, te: event::TransferEvent) {
        let endpoint_id = te.endpoint_id() as usize;
        let trb_pointer = te.trb_pointer() as usize;
        let Some(ring) = self.endpoint_rings[endpoint_id - 1].as_mut() else {
            return;
        };
        if ring.should_skip(trb_pointer) {
            return;
        }
        // Update Transfer Ring Dequeue Pointer.
        ring.set_dequeue_pointer(trb_pointer);

        let slot_state = self.input_context.output_device().slot().slot_state();
        match slot_state {
            SlotState::DisabledEnabled => {
                unreachable!("slot should not be in DisabledEnabled state")
            }
            SlotState::Default => {
                if self.is_full_speed() && endpoint_id == 1 && self.fs_evaluated == false {
                    self.evaluate_context_for_fs();
                }
            }
            SlotState::Addressed => crate::early_println!("TODO: handle addressed transfer event"),
            SlotState::Configured => {
                crate::early_println!("TODO: handle configured transfer event");
                let des = self.extra_data::<DeviceDescriptor>();
                crate::early_println!("{:?}", des);
            }
        };
    }

    fn evaluate_context_for_fs(&mut self) {
        let max_packer_size = {
            let des = self.extra_data::<DeviceDescriptor>();
            des.max_packet_size
        };
        let ep0 = self.input_context.handle_mut().device_mut().endpoint_mut(1);
        ep0.set_max_packet_size(max_packer_size as u16);

        let mut evaluate = command::EvaluateContext::default();
        evaluate.set_slot_id(self.slot_id);
        evaluate.set_input_context_pointer(self.input_context_base() as u64);
        let allowed = command::Allowed::EvaluateContext(evaluate);

        self.pending_commands.get_or_insert_default().push(allowed);
        self.fs_evaluated = true;
    }

    fn set_configuration() {
        // To set a configuration in a device, software shall issue a
        // Configure Endpoint Command to the xHC in conjunction with issuing USB
        // SET_CONFIGURATION request to the device.
    }

    /*
     *  DeviceDescriptor {
     *      length: 18, typ: 1, usb_bcd: 512, class: 0, sub_class: 0, protocol: 0,
     *      max_packet_size: 8, vendor_id: 1575, product_id: 1, device_bcd: 0, manufacturer_index: 1,
     *      product_index: 4, serial_number_index: 5, nr_configurations: 1
     *  }
     */
    /// This request returns the specified descriptor if the descriptor exists.
    ///  - request type: 0b1000_0000
    ///  - request: GET_DESCRIPTOR (6)
    ///  - value: Descriptor Type and Descriptor Index (0x0100)
    ///  - index: Zero or Language ID
    ///  - length: Descriptor Length
    ///  - data: Descriptor
    fn get_device_descriptor(&mut self) {
        // Initialize the Setup Stage TD.
        let mut setup = transfer::SetupStage::default();
        setup.set_transfer_type(TransferType::In);
        setup.clear_interrupt_on_completion();
        setup.set_request_type(0x80); // Dir = Device-to-Host, Type = Standard, Recipient = Device
        setup.set_request(6); // GET_DESCRIPTOR
        setup.set_value(0x0100); // Descriptor Index = 0, Descriptor Type = 1
        setup.set_index(0);
        setup.set_length(size_of::<DeviceDescriptor>() as u16);
        let allowed = transfer::Allowed::SetupStage(setup);
        self.ctrl_ring().enqueue(allowed).unwrap();

        // Initialize the Data Stage TD.
        let mut data = transfer::DataStage::default();
        data.set_direction(transfer::Direction::In);
        data.set_trb_transfer_length(size_of::<DeviceDescriptor>() as u32);
        data.clear_chain_bit();
        data.clear_interrupt_on_completion();
        data.clear_immediate_data();
        data.set_data_buffer_pointer(self.extra_buffer.start_paddr() as u64);
        let allowed = transfer::Allowed::DataStage(data);
        self.ctrl_ring().enqueue(allowed).unwrap();

        // Initialize the Status Stage TD.
        let mut status = transfer::StatusStage::default();
        status.clear_direction();
        status.clear_chain_bit();
        status.set_interrupt_on_completion();
        let allowed = transfer::Allowed::StatusStage(status);
        self.ctrl_ring().enqueue(allowed).unwrap();

        self.pending_doorbell.replace((1, 0)); // DB Target = Control EP 0 Enqueue Pointer Update.
    }
}
