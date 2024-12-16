use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
    vec::Vec,
};

use spin::Once;
use xhci::{
    context::{EndpointState, SlotState},
    ring::trb::{
        event::{CommandCompletion, TransferEvent},
        transfer,
    },
};

use super::{
    slot::{CommandCompletionCallback, TransferEventCallback},
    BusProbeError, DeviceDescriptor, EndpointDescriptor, InterfaceDescriptor, Request,
};
use crate::{
    bus::usb::USB_KBD,
    mm::{paddr_to_vaddr, DmaCoherent, FrameAllocOptions},
    sync::SpinLock,
};

pub trait UsbDevice: Send + Sync {
    fn device(&self) -> DeviceDescriptor;
    fn slot_state(&self) -> SlotState;
    fn interfaces(&self) -> Vec<InterfaceDescriptor>;
    fn endpoints(&self) -> Vec<EndpointDescriptor>;
    fn enable_endpoint(&self, endpoint: &EndpointDescriptor);
    fn endpoint_state(&self, endpoint: &EndpointDescriptor) -> EndpointState;
    fn send_device_request(&self, request: Request);
    fn send_endpoint_request(&self, endpoint: &EndpointDescriptor, allowed: transfer::Allowed);
    fn register_completion_callback(&self, callback: Box<CommandCompletionCallback>);
    fn register_event_callback(&self, callback: Box<TransferEventCallback>);
}

pub trait UsbClass {
    fn probe(&self, device: Weak<dyn UsbDevice>) -> Result<(), BusProbeError>;
    fn init(&self, device: Weak<dyn UsbDevice>);
}

pub struct UsbKeyboardDriver {
    devices: SpinLock<Vec<Weak<dyn UsbDevice>>>,
    dma_buffer: DmaCoherent,
}

impl UsbKeyboardDriver {
    pub fn new() -> Self {
        let seg = FrameAllocOptions::new(1).alloc_contiguous().unwrap();
        let dma_buffer = DmaCoherent::map(seg, true).unwrap();

        Self {
            devices: SpinLock::new(Vec::new()),
            dma_buffer,
        }
    }

    pub fn send_normal(&self) {
        let mut normal = transfer::Normal::default();
        normal.set_data_buffer_pointer(self.dma_buffer.start_paddr() as u64);
        normal.set_trb_transfer_length(8);
        normal.set_interrupt_on_completion();
        normal.clear_chain_bit();
        let allowed = transfer::Allowed::Normal(normal);

        let devices = self.devices.lock();
        for device in devices.iter() {
            let Some(device) = device.upgrade() else {
                continue;
            };

            for endpoint in device.endpoints().iter() {
                device.send_endpoint_request(endpoint, allowed.clone());
            }
        }
    }

    pub fn dma_buffer(&self) -> &[u8] {
        let va = paddr_to_vaddr(self.dma_buffer.start_paddr());
        unsafe { core::slice::from_raw_parts(va as *const u8, 8) }
    }
}

impl UsbClass for UsbKeyboardDriver {
    fn probe(&self, device: Weak<dyn UsbDevice>) -> Result<(), BusProbeError> {
        let Some(device) = device.upgrade() else {
            return Err(BusProbeError::DeviceNotMatch);
        };

        for interface in device.interfaces().iter() {
            if interface.class == 3 && interface.subclass == 1 && interface.protocol == 1 {
                return Ok(());
            }
        }

        return Err(BusProbeError::DeviceNotMatch);
    }

    fn init(&self, device: Weak<dyn UsbDevice>) {
        let Some(device) = device.upgrade() else {
            return;
        };

        device.register_completion_callback(Box::new(handle_command_completion));

        device.register_event_callback(Box::new(handle_transfer_event));

        for endpoint in device.endpoints().iter() {
            device.enable_endpoint(endpoint);
        }

        self.devices.lock().push(Arc::downgrade(&device));
    }
}

fn handle_command_completion(completion: &CommandCompletion) {
    crate::early_println!("====handle command completion at usb keyboard");
    let Some(kbd) = USB_KBD.get() else {
        return;
    };

    kbd.send_normal();
}

fn handle_transfer_event(event: &TransferEvent) {
    crate::early_println!("====handle transfer event at usb keyboard");
    let Some(kbd) = USB_KBD.get() else {
        return;
    };

    crate::early_println!("{:?}", kbd.dma_buffer());
    kbd.send_normal();
}
