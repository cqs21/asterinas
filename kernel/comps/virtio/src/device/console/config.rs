// SPDX-License-Identifier: MPL-2.0

use aster_util::safe_ptr::SafePtr;
use ostd::{io_mem::IoMem, Pod};

use crate::{device::VirtioConfigManager, transport::VirtioTransport};

bitflags::bitflags! {
    pub struct ConsoleFeatures: u64{
        /// Configuration cols and rows are valid.
        const VIRTIO_CONSOLE_F_SIZE = 1 << 0;
        /// Device has support for multiple ports;
        /// max_nr_ports is valid and control virtqueues will be used.
        const VIRTIO_CONSOLE_F_MULTIPORT = 1 << 1;
        /// Device has support for emergency write.
        /// Configuration field emerg_wr is valid.
        const VIRTIO_CONSOLE_F_EMERG_WRITE = 1 << 2;
    }
}

#[derive(Debug, Default, Pod, Clone, Copy)]
#[repr(C)]
pub struct VirtioConsoleConfig {
    pub cols: u16,
    pub rows: u16,
    pub max_nr_ports: u32,
    pub emerg_wr: u32,
}

impl VirtioConfigManager<VirtioConsoleConfig> {
    pub(super) fn from_bar(&self) -> Option<VirtioConsoleConfig> {
        let Some(bar) = self.raw_bar.as_ref() else {
            return None;
        };
        let offset = self.device_config_offset;

        let mut console_config = VirtioConsoleConfig::default();
        // Only following fields are defined in legacy interface.
        console_config.cols = bar.read_val::<u16>(offset).unwrap();
        console_config.rows = bar.read_val::<u16>(offset + 0x2).unwrap();
        console_config.max_nr_ports = bar.read_val::<u32>(offset + 0x4).unwrap();

        Some(console_config)
    }
}
