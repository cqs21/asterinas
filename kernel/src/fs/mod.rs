// SPDX-License-Identifier: MPL-2.0

pub mod cgroupfs;
pub mod device;
pub mod devpts;
pub mod epoll;
pub mod exfat;
pub mod ext2;
pub mod file_handle;
pub mod file_table;
pub mod fs_resolver;
pub mod inode_handle;
pub mod named_pipe;
pub mod overlayfs;
pub mod path;
pub mod pipe;
pub mod procfs;
pub mod ramfs;
pub mod registry;
pub mod rootfs;
pub mod sysfs;
pub mod thread_info;
pub mod utils;

use aster_virtio::device::block::device::BlockDevice as VirtIoBlockDevice;

use crate::prelude::*;

fn start_block_device(device_name: &str) {
    let Some(device) = aster_block::get_device(device_name) else {
        warn!("Device {} not found", device_name);
        return;
    };

    let task_fn = move || {
        info!("spawn the virt-io-block thread");
        let virtio_block_device = device.downcast_ref::<VirtIoBlockDevice>().unwrap();
        loop {
            virtio_block_device.handle_requests();
        }
    };
    crate::ThreadOptions::new(task_fn).spawn();
}

pub fn lazy_init() {
    registry::init();

    sysfs::init();
    procfs::init();
    cgroupfs::init();
    ramfs::init();
    devpts::init();

    ext2::init();
    exfat::init();
    overlayfs::init();

    //The device name is specified in qemu args as --serial={device_name}
    let ext2_device_name = "vext2";
    let exfat_device_name = "vexfat";

    start_block_device(ext2_device_name);

    start_block_device(exfat_device_name);
}
