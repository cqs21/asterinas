// SPDX-License-Identifier: MPL-2.0

//! The framebuffer console of Asterinas.
#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use aster_console::{AnyConsoleDevice, ConsoleCallback};
use aster_keyboard::Key;
use component::{init_component, ComponentInitError};
use ostd::{bus::usb, mm::VmReader, sync::SpinLock};
use spin::Once;

pub static CONSOLE_NAME: &str = "Framebuffer-Console";

static FRAMEBUFFER_CONSOLE_CALLBACKS: Once<SpinLock<Vec<&'static ConsoleCallback>>> = Once::new();

#[init_component]
fn framebuffer_console_init() -> Result<(), ComponentInitError> {
    FRAMEBUFFER_CONSOLE_CALLBACKS.call_once(|| SpinLock::new(Vec::new()));
    aster_keyboard::register_callback(&handle_keyboard_input);
    ostd::bus::usb::register_callback(&handle_usb_keyboard);
    Ok(())
}

#[derive(Debug, Default)]
pub struct FramebufferConsole;

impl FramebufferConsole {
    pub fn new() -> Self {
        Self
    }
}

impl AnyConsoleDevice for FramebufferConsole {
    fn send(&self, buf: &[u8]) {
        // TODO: handle ANSI escape characters
        for &ch in buf.iter() {
            if ch != 0 {
                let char = char::from_u32(ch as u32).unwrap();
                ostd::arch::framebuffer::print(format_args!("{}", char));
            }
        }
    }

    fn register_callback(&self, callback: &'static ConsoleCallback) {
        let Some(callbacks) = FRAMEBUFFER_CONSOLE_CALLBACKS.get() else {
            return;
        };

        callbacks.disable_irq().lock().push(callback);
    }
}

fn handle_keyboard_input(key: Key) {
    let Some(callbacks) = FRAMEBUFFER_CONSOLE_CALLBACKS.get() else {
        return;
    };

    let mut char = [0u8];
    let buffer = match key {
        Key::Char(ch) | Key::Ctrl(ch) => {
            char[0] = ch as u8;
            char.as_slice()
        }
        Key::Enter => [0xD].as_slice(),
        Key::BackSpace => [0x7F].as_slice(),
        Key::Escape => [0x1B].as_slice(),
        Key::Up => [0x1B, 0x5B, 0x41].as_slice(),
        Key::Down => [0x1B, 0x5B, 0x42].as_slice(),
        Key::Right => [0x1B, 0x5B, 0x43].as_slice(),
        Key::Left => [0x1B, 0x5B, 0x44].as_slice(),
        _ => {
            log::debug!("unsupported keyboard input");
            return;
        }
    };

    for callback in callbacks.disable_irq().lock().iter() {
        let reader = VmReader::from(buffer);
        callback(reader);
    }
}

fn handle_usb_keyboard(report: &usb::KeyboardReport) {
    static HAS_MODIFIER: AtomicBool = AtomicBool::new(false);
    static HAS_ARROWS: AtomicBool = AtomicBool::new(false);

    // ostd::early_println!("===={:?}", report);
    let modifier = report.modifier;

    if HAS_MODIFIER.load(Ordering::Acquire) && modifier == usb::Modifier::None {
        HAS_MODIFIER.store(false, Ordering::Release);
        return;
    }

    if modifier != usb::Modifier::None {
        HAS_MODIFIER.store(true, Ordering::Release);
    }

    if report.keys[1] != usb::Key::None {
        return;
    }

    let key = report.keys[0];

    if key == usb::Key::None
        || key == usb::Key::PostFail
        || key == usb::Key::ErrorRollOver
        || key == usb::Key::ErrorUndefined
    {
        return;
    }

    let has_arrows = HAS_ARROWS.load(Ordering::Acquire);
    if has_arrows
        && (key == usb::Key::UpArrow
            || key == usb::Key::DownArrow
            || key == usb::Key::LeftArrow
            || key == usb::Key::RightArrow)
    {
        HAS_ARROWS.store(false, Ordering::Release);
        return;
    }
    if key == usb::Key::UpArrow
        || key == usb::Key::DownArrow
        || key == usb::Key::LeftArrow
        || key == usb::Key::RightArrow
    {
        HAS_ARROWS.store(true, Ordering::Release);
    }

    let key = transfer_usbkey(modifier, key);
    handle_keyboard_input(key);
}

fn transfer_usbkey(modifier: usb::Modifier, key: usb::Key) -> Key {
    let modify = |lower: char, higher: char| -> Key {
        match modifier {
            usb::Modifier::LeftCtrl | usb::Modifier::RightCtrl => Key::Ctrl(lower),
            usb::Modifier::LeftAlt | usb::Modifier::RightAlt => Key::Alt(lower),
            usb::Modifier::LeftShift | usb::Modifier::RightShift => Key::Char(higher),
            _ => Key::Char(lower),
        }
    };

    match key {
        usb::Key::None
        | usb::Key::ErrorRollOver
        | usb::Key::PostFail
        | usb::Key::ErrorUndefined => Key::Null,
        usb::Key::Aa => modify('a', 'A'),
        usb::Key::Bb => modify('b', 'B'),
        usb::Key::Cc => modify('c', 'C'),
        usb::Key::Dd => modify('d', 'D'),
        usb::Key::Ee => modify('e', 'E'),
        usb::Key::Ff => modify('f', 'F'),
        usb::Key::Gg => modify('g', 'G'),
        usb::Key::Hh => modify('h', 'H'),
        usb::Key::Ii => modify('i', 'I'),
        usb::Key::Jj => modify('j', 'J'),
        usb::Key::Kk => modify('k', 'K'),
        usb::Key::Ll => modify('l', 'L'),
        usb::Key::Mm => modify('m', 'M'),
        usb::Key::Nn => modify('n', 'N'),
        usb::Key::Oo => modify('o', 'O'),
        usb::Key::Pp => modify('p', 'P'),
        usb::Key::Qq => modify('q', 'Q'),
        usb::Key::Rr => modify('r', 'R'),
        usb::Key::Ss => modify('s', 'S'),
        usb::Key::Tt => modify('t', 'T'),
        usb::Key::Uu => modify('u', 'U'),
        usb::Key::Vv => modify('v', 'V'),
        usb::Key::Ww => modify('w', 'W'),
        usb::Key::Xx => modify('x', 'X'),
        usb::Key::Yy => modify('y', 'Y'),
        usb::Key::Zz => modify('z', 'Z'),
        usb::Key::Exclamation1 => modify('1', '!'),
        usb::Key::At2 => modify('2', '@'),
        usb::Key::Hash3 => modify('3', '#'),
        usb::Key::Dollar4 => modify('4', '$'),
        usb::Key::Percent5 => modify('5', '%'),
        usb::Key::Caret6 => modify('6', '^'),
        usb::Key::Ampersand7 => modify('7', '&'),
        usb::Key::Asterisk8 => modify('8', '*'),
        usb::Key::LeftParen9 => modify('9', '('),
        usb::Key::RightParen0 => modify('0', ')'),
        usb::Key::Enter => Key::Enter,
        usb::Key::Escape => Key::Escape,
        usb::Key::Backspace => Key::BackSpace,
        usb::Key::Tab => Key::Char('\t'),
        usb::Key::Space => Key::Char(' '),
        usb::Key::MinusDash => modify('-', '_'),
        usb::Key::PlusEqual => modify('=', '+'),
        usb::Key::LeftBraceBracket => modify('[', '{'),
        usb::Key::RightBraceBracket => modify(']', '}'),
        usb::Key::PipeBackslash => modify('\\', '|'),
        usb::Key::ColonSemicolon => modify(';', ':'),
        usb::Key::DoubleSingleQuote => modify('"', '\''),
        usb::Key::TildeBacktick => modify('`', '~'),
        usb::Key::LessThanComma => modify(',', '<'),
        usb::Key::GreaterThanPeriod => modify('.', '>'),
        usb::Key::QuestionSlash => modify('/', '?'),
        usb::Key::F1 => Key::Fn(1),
        usb::Key::F2 => Key::Fn(2),
        usb::Key::F3 => Key::Fn(3),
        usb::Key::F4 => Key::Fn(4),
        usb::Key::F5 => Key::Fn(5),
        usb::Key::F6 => Key::Fn(6),
        usb::Key::F7 => Key::Fn(7),
        usb::Key::F8 => Key::Fn(8),
        usb::Key::F9 => Key::Fn(9),
        usb::Key::F10 => Key::Fn(10),
        usb::Key::F11 => Key::Fn(11),
        usb::Key::F12 => Key::Fn(12),
        usb::Key::Home => Key::Home,
        usb::Key::Delete => Key::Delete,
        usb::Key::End => Key::End,
        usb::Key::RightArrow => Key::Right,
        usb::Key::LeftArrow => Key::Left,
        usb::Key::DownArrow => Key::Down,
        usb::Key::UpArrow => Key::Up,
        _ => Key::Null,
    }
}
