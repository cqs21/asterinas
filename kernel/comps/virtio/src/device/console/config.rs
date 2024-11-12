// SPDX-License-Identifier: MPL-2.0

use core::mem::offset_of;

use aster_util::safe_ptr::SafePtr;
use ostd::Pod;

use crate::transport::{ConfigManager, VirtioTransport};

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

#[derive(Debug, Pod, Clone, Copy)]
#[repr(C)]
pub struct VirtioConsoleConfig {
    pub cols: u16,
    pub rows: u16,
    pub max_nr_ports: u32,
    pub emerg_wr: u32,
}

impl VirtioConsoleConfig {
    pub(super) fn new(transport: &dyn VirtioTransport) -> Self {
        let safe_ptr = transport
            .device_config_mem()
            .map(|mem| SafePtr::new(mem, 0));
        let bar_space = transport.device_config_bar();

        let config_manager = ConfigManager::<VirtioConsoleConfig>::new(safe_ptr, bar_space);

        let mut console_config = VirtioConsoleConfig::new_uninit();
        // Only following fields are defined in legacy interface.
        console_config.cols = config_manager
            .read_once::<u16>(offset_of!(Self, cols))
            .unwrap();
        console_config.rows = config_manager
            .read_once::<u16>(offset_of!(Self, rows))
            .unwrap();
        console_config.max_nr_ports = config_manager
            .read_once::<u32>(offset_of!(Self, max_nr_ports))
            .unwrap();

        if config_manager.is_modern() {
            console_config.emerg_wr = config_manager
                .read_once::<u32>(offset_of!(Self, emerg_wr))
                .unwrap();
        }

        console_config
    }
}
