// SPDX-License-Identifier: MPL-2.0

use alloc::{sync::Arc, vec::Vec};
use core::ops::Deref;

use aster_console::{AnyConsoleDevice, ConsoleCallback};
use aster_keyboard::InputKey;
use font8x8::UnicodeFonts;
use ostd::{
    mm::VmReader,
    sync::{LocalIrqDisabled, SpinLock},
};
use spin::Once;

use crate::{FrameBuffer, Pixel, FRAMEBUFFER};

/// A text console rendered onto the framebuffer.
pub struct FramebufferConsole {
    state: SpinLock<ConsoleState, LocalIrqDisabled>,
    callbacks: SpinLock<Vec<&'static ConsoleCallback>, LocalIrqDisabled>,
}

pub static CONSOLE_NAME: &str = "Framebuffer-Console";

pub static FRAMEBUFFER_CONSOLE: Once<Arc<FramebufferConsole>> = Once::new();

pub(crate) fn init() {
    let Some(fb) = FRAMEBUFFER.get() else {
        log::warn!("Framebuffer not initialized");
        return;
    };

    FRAMEBUFFER_CONSOLE.call_once(|| Arc::new(FramebufferConsole::new(fb.clone())));
    aster_keyboard::register_callback(&handle_keyboard_input);
}

impl AnyConsoleDevice for FramebufferConsole {
    fn send(&self, buf: &[u8]) {
        self.send_buf(buf);
    }

    fn register_callback(&self, callback: &'static ConsoleCallback) {
        self.callbacks.lock().push(callback);
    }
}

impl FramebufferConsole {
    /// Creates a new framebuffer console.
    pub fn new(framebuffer: Arc<FrameBuffer>) -> Self {
        let bytes = alloc::vec![0u8; framebuffer.size()];
        Self {
            state: SpinLock::new(ConsoleState {
                enabled: true,
                x_pos: 0,
                y_pos: 0,
                fg_color: Pixel::WHITE,
                bg_color: Pixel::BLACK,
                bytes,
                backend: framebuffer,
            }),
            callbacks: SpinLock::new(Vec::new()),
        }
    }

    /// Returns whether the console is enabled.
    pub fn is_enabled(&self) -> bool {
        self.state.lock().enabled
    }

    /// Enables the console.
    pub fn enable(&self) {
        self.state.lock().enabled = true;
    }

    /// Disables the console.
    pub fn disable(&self) {
        self.state.lock().enabled = false;
    }

    /// Returns the current cursor position.
    pub fn cursor(&self) -> (usize, usize) {
        let state = self.state.lock();
        (state.x_pos, state.y_pos)
    }

    /// Sets the cursor position.
    pub fn set_cursor(&self, x: usize, y: usize) {
        let mut state = self.state.lock();
        if x > state.backend.width() - 8 || y > state.backend.height() - 8 {
            log::warn!("Invalid framebuffer cursor position: ({}, {})", x, y);
            return;
        }
        state.x_pos = x;
        state.y_pos = y;
    }

    /// Returns the foreground color.
    pub fn fg_color(&self) -> Pixel {
        self.state.lock().fg_color
    }

    /// Sets the foreground color.
    pub fn set_fg_color(&self, val: Pixel) {
        self.state.lock().fg_color = val;
    }

    /// Returns the background color.
    pub fn bg_color(&self) -> Pixel {
        self.state.lock().bg_color
    }

    /// Sets the background color.
    pub fn set_bg_color(&self, val: Pixel) {
        self.state.lock().bg_color = val;
    }

    /// Sends a single character to be drawn on the framebuffer.
    pub fn send_char(&self, c: char) {
        let mut state = self.state.lock();
        if !state.enabled {
            return;
        }

        if c == '\n' {
            state.newline();
            return;
        } else if c == '\r' {
            state.carriage_return();
            return;
        }

        if state.x_pos >= state.backend.width() {
            state.newline();
        }

        let rendered = font8x8::BASIC_FONTS
            .get(c)
            .expect("character not found in basic font");
        for (y, byte) in rendered.iter().enumerate() {
            for (x, bit) in (0..8).enumerate() {
                let x = state.x_pos + x;
                let y = state.y_pos + y;
                let on = *byte & (1 << bit) != 0;
                let pixel = if on { state.fg_color } else { state.bg_color };

                // Cache the rendered pixel
                let pixel_format = state.backend.pixel_format();
                let offset = (y * state.backend.width() + x) * pixel_format.nbytes();
                let _ = pixel.render(pixel_format, &mut state.bytes[offset..]);

                // Write the pixel to the framebuffer
                state.backend.write_pixel_at(x, y, pixel);
            }
        }
        state.x_pos += 8;
    }

    /// Sends a buffer of bytes to be drawn on the framebuffer.
    ///
    /// # Panics
    ///
    /// This method will panic if any byte in the buffer cannot be converted
    /// into a valid Unicode character.
    pub fn send_buf(&self, buf: &[u8]) {
        // TODO: handle ANSI escape sequences.
        for &byte in buf.iter() {
            if byte != 0 {
                let char = char::from_u32(byte as u32).unwrap();
                self.send_char(char);
            }
        }
    }
}

impl core::fmt::Debug for FramebufferConsole {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FramebufferConsole").finish()
    }
}

#[derive(Debug)]
struct ConsoleState {
    enabled: bool,
    x_pos: usize,
    y_pos: usize,
    fg_color: Pixel,
    bg_color: Pixel,
    bytes: Vec<u8>,
    backend: Arc<FrameBuffer>,
}

impl ConsoleState {
    fn carriage_return(&mut self) {
        self.x_pos = 0;
    }

    fn newline(&mut self) {
        if self.y_pos >= self.backend.height() - 8 {
            self.shift_lines_up();
        }
        self.y_pos += 8;
        self.x_pos = 0;
    }

    fn shift_lines_up(&mut self) {
        let offset = self.backend.width() * self.backend.pixel_format().nbytes() * 8;
        self.bytes.copy_within(offset.., 0);
        self.bytes[self.backend.size() - offset..].fill(0);
        self.backend.write_bytes_at(0, &self.bytes);
        self.y_pos -= 8;
    }
}

fn handle_keyboard_input(key: InputKey) {
    if key == InputKey::Nul {
        return;
    }

    let Some(console) = FRAMEBUFFER_CONSOLE.get() else {
        return;
    };

    let buffer = key.deref();
    for callback in console.callbacks.lock().iter() {
        let reader = VmReader::from(buffer);
        callback(reader);
    }
}
