// SPDX-License-Identifier: MPL-2.0

//! Provides i8042 PC keyboard I/O port access.

use crate::{
    arch::x86::{
        device::io_port::{IoPort, ReadOnlyAccess},
        kernel::{pic, IO_APIC},
    },
    trap::IrqLine,
};

/// Keyboard data register (R/W)
pub static KEYBOARD_DATA_PORT: IoPort<u8, ReadOnlyAccess> = unsafe { IoPort::new(0x60) };

/// Keyboard status register (R)
pub static KEYBOARD_STATUS_PORT: IoPort<u8, ReadOnlyAccess> = unsafe { IoPort::new(0x64) };

/// Alloc IrqLine for keyboard, then user could register callbacks via IrqLine::on_active.
pub fn alloc_keyboard_irq() -> IrqLine {
    let irq = if !IO_APIC.is_completed() {
        pic::allocate_irq(4).unwrap()
    } else {
        let irq = IrqLine::alloc().unwrap();
        let mut io_apic = IO_APIC.get().unwrap().first().unwrap().lock();
        io_apic.enable(1, irq.clone()).unwrap();
        irq
    };

    irq
}
