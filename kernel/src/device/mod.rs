// SPDX-License-Identifier: MPL-2.0

mod char;
mod disk;
mod mem;
pub mod misc;
mod pty;
mod shm;
pub mod tty;

use alloc::format;

use device_id::DeviceId;
pub use mem::{getrandom, geturandom};
pub use pty::{new_pty_pair, PtyMaster, PtySlave};

use crate::{
    fs::{
        device::{add_node, Device},
        fs_resolver::FsPath,
        path::PerMountFlags,
        ramfs::RamFs,
    },
    prelude::*,
};

pub fn init_in_first_kthread() {
    disk::init_in_first_kthread();
    mem::init_in_first_kthread();
    misc::init_in_first_kthread();
}

/// Init the device node in fs, must be called after mounting rootfs.
pub fn init_in_first_process(ctx: &Context) -> Result<()> {
    let fs = ctx.thread_local.borrow_fs();
    let fs_resolver = fs.resolver().read();

    // Mount DevFS
    let dev_path = fs_resolver.lookup(&FsPath::try_from("/dev")?)?;
    dev_path.mount(RamFs::new(), PerMountFlags::default(), ctx)?;

    char::init_in_first_process(&fs_resolver)?;

    tty::init();

    let tty = Arc::new(tty::TtyDevice);
    add_node(tty, "tty", &fs_resolver)?;

    let console = tty::system_console().clone();
    add_node(console, "console", &fs_resolver)?;

    for (index, tty) in tty::iter_n_tty().enumerate() {
        add_node(tty.clone(), &format!("tty{}", index), &fs_resolver)?;
    }

    pty::init_in_first_process(&fs_resolver, ctx)?;

    shm::init_in_first_process(&fs_resolver, ctx)?;

    disk::init_in_first_process(&fs_resolver)?;

    Ok(())
}

// TODO: Implement a more scalable solution for ID-to-device mapping.
// Instead of hardcoding every device numbers in this function,
// a registration mechanism should be used to allow each driver to
// allocate device IDs either statically or dynamically.
pub fn get_device(id: DeviceId) -> Result<Arc<dyn Device>> {
    char::lookup(id)
        .map(|char| char.as_device())
        .ok_or(Error::with_message(
            Errno::EINVAL,
            "the device ID is invalid or unsupported",
        ))
}
