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
pub mod tmpfs;
pub mod utils;

use crate::{
    fs::{
        file_table::FdFlags,
        fs_resolver::{FsPath, FsResolver},
        utils::{AccessMode, InodeMode},
    },
    prelude::*,
};

pub fn init() {
    registry::init();

    sysfs::init();
    procfs::init();
    cgroupfs::init();
    ramfs::init();
    tmpfs::init();
    devpts::init();

    ext2::init();
    exfat::init();
    overlayfs::init();

    rootfs::init();
}

pub fn init_in_first_kthread(fs_resolver: &FsResolver) {
    rootfs::init_in_first_kthread(fs_resolver).unwrap();
}

pub fn init_in_first_process(ctx: &Context) {
    let fs = ctx.thread_local.borrow_fs();
    let fs_resolver = fs.resolver().read();

    // Initialize the file table for the first process.
    let tty_path = FsPath::new(fs_resolver::AT_FDCWD, "/dev/console").expect("cannot find tty");
    let stdin = {
        let flags = AccessMode::O_RDONLY as u32;
        let mode = InodeMode::S_IRUSR;
        fs_resolver.open(&tty_path, flags, mode.bits()).unwrap()
    };
    let stdout = {
        let flags = AccessMode::O_WRONLY as u32;
        let mode = InodeMode::S_IWUSR;
        fs_resolver.open(&tty_path, flags, mode.bits()).unwrap()
    };
    let stderr = {
        let flags = AccessMode::O_WRONLY as u32;
        let mode = InodeMode::S_IWUSR;
        fs_resolver.open(&tty_path, flags, mode.bits()).unwrap()
    };

    let mut file_table_ref = ctx.thread_local.borrow_file_table_mut();
    let mut file_table = file_table_ref.unwrap().write();

    file_table.insert(Arc::new(stdin), FdFlags::empty());
    file_table.insert(Arc::new(stdout), FdFlags::empty());
    file_table.insert(Arc::new(stderr), FdFlags::empty());
}
