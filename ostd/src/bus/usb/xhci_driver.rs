use alloc::{boxed::Box, sync::Arc, vec::Vec};

use super::XhciDevice;
use crate::{
    bus::{
        pci::{
            bus::{PciDevice, PciDriver},
            common_device::PciCommonDevice,
        },
        BusProbeError,
    },
    sync::SpinLock,
};

#[derive(Debug)]
pub struct XhciDriver {
    pub devices: SpinLock<Vec<XhciDevice>>,
}

impl XhciDriver {
    pub fn new() -> Self {
        Self {
            devices: SpinLock::new(Vec::new()),
        }
    }
}

impl PciDriver for XhciDriver {
    fn probe(
        &self,
        device: PciCommonDevice,
    ) -> Result<Arc<dyn PciDevice>, (BusProbeError, PciCommonDevice)> {
        let device_id = device.device_id();
        if device_id.class != 0x0C || device_id.subclass != 0x03 || device_id.prog_if != 0x30 {
            return Err((BusProbeError::DeviceNotMatch, device));
        }

        XhciDevice::init(device)
    }
}
