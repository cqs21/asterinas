// SPDX-License-Identifier: MPL-2.0

use alloc::sync::Arc;

use ostd::{boot::boot_info, io::IoMem, mm::VmIo};
use spin::Once;

/// The framebuffer used for text or graphical output.
///
/// # Notes
///
/// It is highly recommended to use a synchronization primitive, such as a `SpinLock`, to
/// lock the framebuffer before performing any operation on it.
/// Failing to properly synchronize access can result in corrupted framebuffer content
/// or undefined behavior during rendering.
#[derive(Debug)]
pub struct FrameBuffer {
    io_mem: IoMem,
    width: usize,
    height: usize,
    pixel_format: PixelFormat,
}

/// Pixel format that defines the memory layout of each pixel in the framebuffer.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PixelFormat {
    /// Each pixel uses 8 bits to represent its grayscale intensity, ranging from 0 (black) to 255 (white).
    Grayscale8,
    /// Each pixel uses 16 bits, with 5 bits for Red, 6 bits for Green, and 5 bits for Blue.
    Rgb565,
    /// Each pixel uses 24 bits, with 8 bits for Red, 8 bits for Green, and 8 bits for Blue.
    Rgb888,
    /// Each pixel uses 32 bits, with 8 bits each for Red, Green, Blue, and Alpha (transparency).
    Rgba,
    /// Each pixel uses 32 bits, with 8 bits each for Green, Blue, Red, and Alpha (transparency).
    Gbra,
}

/// Individual pixel data containing raw channel values.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Pixel {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

pub static FRAMEBUFFER: Once<Arc<FrameBuffer>> = Once::new();

pub(crate) fn init() {
    let Some(framebuffer_arg) = boot_info().framebuffer_arg else {
        log::warn!("Framebuffer not found");
        return;
    };

    if framebuffer_arg.address == 0 {
        log::error!("Framebuffer address is zero");
        return;
    }

    let pixel_format = match framebuffer_arg.bpp {
        8 => PixelFormat::Grayscale8,
        16 => PixelFormat::Rgb565,
        24 => PixelFormat::Rgb888,
        32 => {
            // FIXME: There are several pixel formats that have the same BPP. We lost the information
            // during the boot phase, so here we guess the pixel format on a best effort basis.
            PixelFormat::Rgba
        }
        _ => {
            log::error!(
                "Unsupported framebuffer pixel format: {} bpp",
                framebuffer_arg.bpp
            );
            return;
        }
    };

    let framebuffer = {
        let fb_base = framebuffer_arg.address;
        let fb_size = framebuffer_arg.width * framebuffer_arg.height * (framebuffer_arg.bpp / 8);
        let io_mem = IoMem::acquire(fb_base..fb_base + fb_size).unwrap();
        FrameBuffer {
            io_mem,
            width: framebuffer_arg.width,
            height: framebuffer_arg.height,
            pixel_format,
        }
    };

    framebuffer.clear();
    FRAMEBUFFER.call_once(|| Arc::new(framebuffer));
}

impl FrameBuffer {
    /// Returns the size of the framebuffer in bytes.
    pub fn size(&self) -> usize {
        self.io_mem.length()
    }

    /// Returns the width of the framebuffer in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the framebuffer in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the pixel format of the framebuffer.
    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    /// Writes a pixel at the specified position.
    pub fn write_pixel_at(&self, x: usize, y: usize, pixel: Pixel) {
        let mut color_bytes = [0u8; 4];
        let pixel = pixel.render(self.pixel_format, &mut color_bytes);

        let bytes_per_pixel = self.pixel_format.nbytes();
        let offset = (y * self.width + x) * bytes_per_pixel;
        self.io_mem.write_bytes(offset, pixel).unwrap();
    }

    /// Writes raw bytes at the specified offset.
    pub fn write_bytes_at(&self, offset: usize, bytes: &[u8]) {
        if offset >= self.io_mem.length() {
            log::warn!("Framebuffer offset out of bounds: {}", offset);
            return;
        }

        let len = bytes.len().min(self.io_mem.length() - offset);
        self.io_mem.write_bytes(offset, &bytes[..len]).unwrap();
    }

    /// Clears the framebuffer with default color (black).
    pub fn clear(&self) {
        let frame = alloc::vec![0u8; self.size()];
        self.write_bytes_at(0, &frame);
    }
}

impl PixelFormat {
    /// Returns the number of bytes per pixel (color depth).
    pub fn nbytes(&self) -> usize {
        match self {
            PixelFormat::Grayscale8 => 1,
            PixelFormat::Rgb565 => 2,
            PixelFormat::Rgb888 => 3,
            PixelFormat::Rgba => 4,
            PixelFormat::Gbra => 4,
        }
    }
}

impl Pixel {
    /// Renders the pixel to a buffer in the specified format.
    ///
    /// # Panics
    ///
    /// This function will panic if the buffer is not large enough to hold the rendered pixel data.
    pub fn render<'a>(&self, format: PixelFormat, buf: &'a mut [u8]) -> &'a [u8] {
        let nbytes = format.nbytes();
        debug_assert!(buf.len() >= nbytes);

        match format {
            PixelFormat::Grayscale8 => {
                // Calculate the grayscale value
                let red_weight = 77 * self.red as u16; // Equivalent to 0.299 * 256
                let green_weight = 150 * self.green as u16; // Equivalent to 0.587 * 256
                let blue_weight = 29 * self.blue as u16; // Equivalent to 0.114 * 256
                let grayscale = (red_weight + green_weight + blue_weight) >> 8; // Normalize to 0-255
                buf[0] = grayscale as u8;
            }
            PixelFormat::Rgb565 => {
                let r = (self.red >> 3) as u16; // Red (5 bits)
                let g = (self.green >> 2) as u16; // Green (6 bits)
                let b = (self.blue >> 3) as u16; // Blue (5 bits)
                                                 // Combine into RGB565 format
                let rgb565 = (r << 11) | (g << 5) | b;
                buf[0..2].copy_from_slice(&rgb565.to_be_bytes());
            }
            PixelFormat::Rgb888 => {
                buf[0] = self.red;
                buf[1] = self.green;
                buf[2] = self.blue;
            }
            PixelFormat::Rgba => {
                buf[0] = self.red;
                buf[1] = self.green;
                buf[2] = self.blue;
                buf[3] = self.alpha;
            }
            PixelFormat::Gbra => {
                buf[0] = self.green;
                buf[1] = self.blue;
                buf[2] = self.red;
                buf[3] = self.alpha;
            }
        }
        &buf[..nbytes]
    }
}

impl Pixel {
    pub const WHITE: Pixel = Pixel {
        red: 0xFF,
        green: 0xFF,
        blue: 0xFF,
        alpha: 0xFF,
    };
    pub const BLACK: Pixel = Pixel {
        red: 0x00,
        green: 0x00,
        blue: 0x00,
        alpha: 0x00,
    };
}
