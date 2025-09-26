// SPDX-License-Identifier: MPL-2.0

mod mem;
mod pty;
mod shm;
pub mod tty;

#[cfg(all(target_arch = "x86_64", feature = "cvm_guest"))]
mod tdxguest;

use alloc::format;

pub use mem::{getrandom, geturandom};
pub use pty::{new_pty_pair, PtyMaster, PtySlave, PTMX};
pub use tty::TTYAUX_ID_ALLOCATOR;

use crate::{
    fs::{
        device::{add_node, all_devices},
        fs_resolver::FsPath,
        ramfs::RamFs,
    },
    prelude::*,
};

/// Init the device node in fs, must be called after mounting rootfs.
pub fn init_in_first_process(ctx: &Context) -> Result<()> {
    let fs = ctx.thread_local.borrow_fs();
    let fs_resolver = fs.resolver().read();

    // Mount DevFS
    let dev_path = fs_resolver.lookup(&FsPath::try_from("/dev")?)?;
    dev_path.mount(RamFs::new(), ctx)?;

    mem::init_in_first_process();
    tty::init_in_first_process();
    pty::init_in_first_process(&fs_resolver, ctx)?;
    shm::init_in_first_process(&fs_resolver, ctx)?;

    for device in all_devices() {
        let path = device.sysnode().name().to_string();
        add_node(device, &path, &fs_resolver)?;
    }

    #[cfg(target_arch = "x86_64")]
    ostd::if_tdx_enabled!({
        add_node(Arc::new(tdxguest::TdxGuest), "tdx_guest", &fs_resolver)?;
    });

    Ok(())
}
