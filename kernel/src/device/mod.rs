// SPDX-License-Identifier: MPL-2.0

use cfg_if::cfg_if;

mod null;
mod pty;
mod random;
pub mod tty;
mod urandom;
mod zero;

cfg_if! {
    if #[cfg(all(target_arch = "x86_64", feature = "cvm_guest"))] {
        mod tdxguest;

        use tdx_guest::tdx_is_enabled;

        pub use tdxguest::TdxGuest;
    }
}

use aster_keyboard::{register_callback, Key};
pub use pty::{new_pty_pair, PtyMaster, PtySlave};
pub use random::Random;
pub use urandom::Urandom;

use self::tty::get_n_tty;
use crate::{
    device::tty::driver::keyboard_input_callback,
    fs::device::{add_node, Device, DeviceId, DeviceType},
    prelude::*,
};

fn keyboard_tty_callback(key: Key) {
    match key {
        Key::Char(ch) => {
            keyboard_input_callback(ch as u8);
        }
        Key::Enter => {
            keyboard_input_callback(0x0D);
        }
        Key::BackSpace => {
            keyboard_input_callback(0x7F);
        }
        Key::Escape => {
            keyboard_input_callback(0x1B);
        }
        Key::Up => {
            keyboard_input_callback(0x1B);
            keyboard_input_callback(0x5B);
            keyboard_input_callback(0x41);
        }
        Key::Down => {
            keyboard_input_callback(0x1B);
            keyboard_input_callback(0x5B);
            keyboard_input_callback(0x42);
        }
        Key::Right => {
            keyboard_input_callback(0x1B);
            keyboard_input_callback(0x5B);
            keyboard_input_callback(0x43);
        }
        Key::Left => {
            keyboard_input_callback(0x1B);
            keyboard_input_callback(0x5B);
            keyboard_input_callback(0x44);
        }
        Key::Ctrl(ch) => {
            keyboard_input_callback(ch as u8);
        }
        Key::Null => return,
        _ => ostd::arch::serial::print(format_args!("unsupported keyboard input")),
    }
}

fn keyboard_tty() {
    register_callback(&keyboard_tty_callback);
}

/// Init the device node in fs, must be called after mounting rootfs.
pub fn init() -> Result<()> {
    let null = Arc::new(null::Null);
    add_node(null, "null")?;
    let zero = Arc::new(zero::Zero);
    add_node(zero, "zero")?;
    tty::init();
    keyboard_tty();
    let console = get_n_tty().clone();
    add_node(console, "console")?;
    let tty = Arc::new(tty::TtyDevice);
    add_node(tty, "tty")?;
    cfg_if! {
        if #[cfg(all(target_arch = "x86_64", feature = "cvm_guest"))] {
            let tdx_guest = Arc::new(tdxguest::TdxGuest);

            if tdx_is_enabled() {
                add_node(tdx_guest, "tdx_guest")?;
            }
        }
    }
    let random = Arc::new(random::Random);
    add_node(random, "random")?;
    let urandom = Arc::new(urandom::Urandom);
    add_node(urandom, "urandom")?;
    pty::init()?;
    Ok(())
}

// TODO: Implement a more scalable solution for ID-to-device mapping.
// Instead of hardcoding every device numbers in this function,
// a registration mechanism should be used to allow each driver to
// allocate device IDs either statically or dynamically.
pub fn get_device(dev: usize) -> Result<Arc<dyn Device>> {
    if dev == 0 {
        return_errno_with_message!(Errno::EPERM, "whiteout device")
    }

    let devid = DeviceId::from(dev as u64);
    let major = devid.major();
    let minor = devid.minor();

    match (major, minor) {
        (1, 3) => Ok(Arc::new(null::Null)),
        (1, 5) => Ok(Arc::new(zero::Zero)),
        (5, 0) => Ok(Arc::new(tty::TtyDevice)),
        (1, 8) => Ok(Arc::new(random::Random)),
        (1, 9) => Ok(Arc::new(urandom::Urandom)),
        _ => return_errno_with_message!(Errno::EINVAL, "unsupported device"),
    }
}
