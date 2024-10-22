// SPDX-License-Identifier: MPL-2.0

use ostd::Pod;

use crate::transport::VirtioTransport;

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

impl VirtioConsoleConfig {
    pub(super) fn new(transport: &dyn VirtioTransport) -> Self {
        let config_manager = transport.device_config();
        if let Ok(console_config) = config_manager.read_config::<Self>() {
            return console_config;
        }

        let mut console_config = VirtioConsoleConfig::default();
        // Only following fields are defined in legacy interface.
        console_config.cols = config_manager.read_once::<u16>(0x0).unwrap();
        console_config.rows = config_manager.read_once::<u16>(0x2).unwrap();
        console_config.max_nr_ports = config_manager.read_once::<u32>(0x4).unwrap();

        console_config
    }
}
