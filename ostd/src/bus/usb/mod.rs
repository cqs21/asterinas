use alloc::sync::{Arc, Weak};

use class::UsbKeyboardDriver;
use requests::Request;
use spin::Once;
use xhci::context::{EndpointState, SlotState};

use super::{pci::PCI_BUS, BusProbeError};

mod class;
mod command_ring;
mod descriptor;
mod device_contexts;
mod event_ring;
mod requests;
mod slot;
mod transfer_ring;
mod xhci_device;
mod xhci_driver;

pub use class::{register_callback, KeyboardReport, UsbClass, UsbDevice};
pub use command_ring::{CommandRing, TRBRingErr, DEFAULT_RING_SIZE};
pub use descriptor::{
    ConfigurationDescriptor, DeviceDescriptor, EndpointDescriptor, InterfaceAssociationDescriptor,
    InterfaceDescriptor,
};
pub use device_contexts::DeviceContexts;
pub use event_ring::EventRingSegmentTable;
pub use slot::XhciSlot;
pub use xhci_device::{DoorbellReason, XHostController};
pub use xhci_driver::XHostControllerDriver;

pub static XHCI_DRIVER: Once<Arc<XHostControllerDriver>> = Once::new();

pub static USB_KBD: Once<Arc<UsbKeyboardDriver>> = Once::new();

pub(super) fn init() {
    XHCI_DRIVER.call_once(|| Arc::new(XHostControllerDriver::new()));
    USB_KBD.call_once(|| Arc::new(UsbKeyboardDriver::new()));
    PCI_BUS
        .lock()
        .register_driver(XHCI_DRIVER.get().unwrap().clone());
}

pub fn register_class_driver(class: Arc<dyn UsbClass>) {
    let Some(xhci) = XHCI_DRIVER.get() else {
        return;
    };

    let mut controllers = xhci.controllers.disable_irq().lock();
    for controller in controllers.iter_mut() {
        controller.register_class_driver(class.clone());
    }
}
