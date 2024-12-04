use alloc::sync::Arc;

use spin::Once;

use super::pci::PCI_BUS;

mod command_ring;
mod descriptor;
mod device_contexts;
mod event_ring;
mod requests;
mod slot;
mod transfer_ring;
mod xhci_device;
mod xhci_driver;

pub use command_ring::{CommandRing, TRBRingErr, DEFAULT_RING_SIZE};
pub use descriptor::{
    ConfigurationDescriptor, DeviceDescriptor, EndpointDescriptor, InterfaceAssociationDescriptor,
    InterfaceDescriptor,
};
pub use device_contexts::DeviceContexts;
pub use event_ring::EventRingSegmentTable;
pub use slot::XhciSlot;
pub use xhci_device::{DoorbellReason, XhciDevice};
pub use xhci_driver::XhciDriver;

pub static XHCI_DRIVER: Once<Arc<XhciDriver>> = Once::new();

pub(super) fn init() {
    XHCI_DRIVER.call_once(|| Arc::new(XhciDriver::new()));
    PCI_BUS
        .lock()
        .register_driver(XHCI_DRIVER.get().unwrap().clone());
}
