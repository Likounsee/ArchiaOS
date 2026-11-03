use core::fmt::{self, Write};
use core::ptr;

use super::{boot_status, framebuffer, port_in, port_out};

const COM1: u16 = 0x3F8;

// Minimal 5x7 text console for early boot. The GOP mode geometry comes from BootInfo v2;
// no resolution or scan-line stride is guessed. The font is intentionally uppercase-only
// so the implementation stays small and has no external font dependency.
pub(super) struct FramebufferConsole {
    pixels: *mut u32,
    width: usize,
    height: usize,
    stride: usize,
    pixel_format: u32,
    x: usize,
    y: usize,
    line: [u8; 192],
    line_len: usize,
}
static mut FRAMEBUFFER_CONSOLE: Option<FramebufferConsole> = None;
// Preserve the very first serial diagnostics until validated GOP geometry lets us
// render them. This is fixed-size and allocation-free for early kernel startup.
const EARLY_LOG_CAPACITY: usize = 4096;
static mut EARLY_LOG_BUFFER: [u8; EARLY_LOG_CAPACITY] = [0; EARLY_LOG_CAPACITY];
static mut EARLY_LOG_LEN: usize = 0;

impl FramebufferConsole {
    pub(super) fn initialize(info: &archiaos_bootloader::bootinfo::BootInfo) -> bool {
        let Some(_required_pixels) = framebuffer::validated_pixel_count(
            info.framebuffer_base,
            info.framebuffer_size,
            info.framebuffer_width,
            info.framebuffer_height,
            info.framebuffer_stride,
            info.framebuffer_pixel_format,
        ) else {
            return false;
        };
        let mut console = Self {
            pixels: info.framebuffer_base as *mut u32,
            width: info.framebuffer_width as usize,
            height: info.framebuffer_height as usize,
            stride: info.framebuffer_stride as usize,
            pixel_format: info.framebuffer_pixel_format,
            x: 8,
            y: 8,
            line: [0; 192],
            line_len: 0,
        };
        console.clear();
        // Publish the initialized renderer before sending the banner through the common
        // serial writer; from this point onward every boot log is mirrored to the monitor.
        unsafe { FRAMEBUFFER_CONSOLE = Some(console); }
        for byte in b"ARCHIAOS KERNEL STARTING...\n" {
            framebuffer_console_write_byte(*byte);
        }
        replay_early_console_log();
        true
    }

    fn color(&self, rgb: u32) -> u32 {
        // The green component is in the same byte in both supported GOP layouts.
        if self.pixel_format == 0 {
            ((rgb & 0x000000ff) << 16) | (rgb & 0x0000ff00) | ((rgb & 0x00ff0000) >> 16)
        } else {
            rgb
        }
    }

    fn clear(&mut self) {
        let total = self.stride.saturating_mul(self.height);
        for i in 0..total {
            // SAFETY: total is bounded by the checked GOP stride * height range.
            unsafe { ptr::write_volatile(self.pixels.add(i), 0x00000000); }
        }
    }

    fn pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height { return; }
        let Some(index) = y.checked_mul(self.stride).and_then(|v| v.checked_add(x)) else { return; };
        // SAFETY: x/y are inside visible resolution and index is inside validated stride * height.
        unsafe { ptr::write_volatile(self.pixels.add(index), self.color(color)); }
    }

    fn scroll(&mut self) {
        const CELL_H: usize = 9;
        if self.height <= CELL_H {
            self.clear();
            self.y = 0;
            return;
        }
        let rows = self.height - CELL_H;
        for y in 0..rows {
            for x in 0..self.width {
                let src = y.saturating_add(CELL_H).saturating_mul(self.stride).saturating_add(x);
                let dst = y.saturating_mul(self.stride).saturating_add(x);
                // SAFETY: source and destination stay within the validated framebuffer.
                let value = unsafe { ptr::read_volatile(self.pixels.add(src)) };
                unsafe { ptr::write_volatile(self.pixels.add(dst), value); }
            }
        }
        for y in rows..self.height {
            for x in 0..self.width {
                let index = y.saturating_mul(self.stride).saturating_add(x);
                unsafe { ptr::write_volatile(self.pixels.add(index), 0); }
            }
        }
        self.y = rows;
    }

    fn newline(&mut self) {
        self.x = 8;
        self.y = self.y.saturating_add(9);
        if self.y.saturating_add(7) >= self.height { self.scroll(); }
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\r' => {}
            b'\n' => self.flush_line(),
            b => {
                if self.line_len == self.line.len() {
                    self.flush_line();
                }
                self.line[self.line_len] = b;
                self.line_len += 1;
            }
        }
    }

    fn flush_line(&mut self) {
        let (prefix, color): (&[u8], u32) = match boot_status::classify(&self.line[..self.line_len]) {
            boot_status::Status::Failure => (b"[FAIL] ", 0x00ff7777),
            boot_status::Status::Success => (b"[ OK ] ", 0x007cf2a0),
            boot_status::Status::Pending => (b"[ .. ] ", 0x00c0c8d0),
        };
        for byte in prefix {
            self.render_byte(*byte, color);
        }
        for index in 0..self.line_len {
            self.render_byte(self.line[index], color);
        }
        self.line_len = 0;
        self.newline();
    }

    fn render_byte(&mut self, byte: u8, color: u32) {
        if byte == b'\t' {
            self.x = self.x.saturating_add(24);
            if self.x.saturating_add(5) >= self.width { self.newline(); }
            return;
        }
        if self.x.saturating_add(5) >= self.width { self.newline(); }
        let glyph = glyph5x7(byte);
        for (gx, column) in glyph.iter().enumerate() {
            for gy in 0..7 {
                if (column >> gy) & 1 != 0 {
                    self.pixel(self.x + gx, self.y + gy, color);
                }
            }
        }
        self.x = self.x.saturating_add(6);
    }
}

fn glyph5x7(byte: u8) -> [u8; 5] {
    match byte.to_ascii_uppercase() {
        b'A' => [0x7e,0x11,0x11,0x11,0x7e], b'B' => [0x7f,0x49,0x49,0x49,0x36],
        b'C' => [0x3e,0x41,0x41,0x41,0x22], b'D' => [0x7f,0x41,0x41,0x22,0x1c],
        b'E' => [0x7f,0x49,0x49,0x49,0x41], b'F' => [0x7f,0x09,0x09,0x09,0x01],
        b'G' => [0x3e,0x41,0x49,0x49,0x7a], b'H' => [0x7f,0x08,0x08,0x08,0x7f],
        b'I' => [0x00,0x41,0x7f,0x41,0x00], b'J' => [0x20,0x40,0x41,0x3f,0x01],
        b'K' => [0x7f,0x08,0x14,0x22,0x41], b'L' => [0x7f,0x40,0x40,0x40,0x40],
        b'M' => [0x7f,0x02,0x0c,0x02,0x7f], b'N' => [0x7f,0x04,0x08,0x10,0x7f],
        b'O' => [0x3e,0x41,0x41,0x41,0x3e], b'P' => [0x7f,0x09,0x09,0x09,0x06],
        b'Q' => [0x3e,0x41,0x51,0x21,0x5e], b'R' => [0x7f,0x09,0x19,0x29,0x46],
        b'S' => [0x46,0x49,0x49,0x49,0x31], b'T' => [0x01,0x01,0x7f,0x01,0x01],
        b'U' => [0x3f,0x40,0x40,0x40,0x3f], b'V' => [0x1f,0x20,0x40,0x20,0x1f],
        b'W' => [0x3f,0x40,0x38,0x40,0x3f], b'X' => [0x63,0x14,0x08,0x14,0x63],
        b'Y' => [0x07,0x08,0x70,0x08,0x07], b'Z' => [0x61,0x51,0x49,0x45,0x43],
        b'0' => [0x3e,0x51,0x49,0x45,0x3e], b'1' => [0x00,0x42,0x7f,0x40,0x00],
        b'2' => [0x42,0x61,0x51,0x49,0x46], b'3' => [0x21,0x41,0x45,0x4b,0x31],
        b'4' => [0x18,0x14,0x12,0x7f,0x10], b'5' => [0x27,0x45,0x45,0x45,0x39],
        b'6' => [0x3c,0x4a,0x49,0x49,0x30], b'7' => [0x01,0x71,0x09,0x05,0x03],
        b'8' => [0x36,0x49,0x49,0x49,0x36], b'9' => [0x06,0x49,0x49,0x29,0x1e],
        b'[' => [0x00,0x7f,0x41,0x41,0x00], b']' => [0x00,0x41,0x41,0x7f,0x00],
        b'(' => [0x00,0x1c,0x22,0x41,0x00], b')' => [0x00,0x41,0x22,0x1c,0x00],
        b':' => [0x00,0x36,0x36,0x00,0x00], b';' => [0x00,0x56,0x36,0x00,0x00],
        b'.' => [0x00,0x60,0x60,0x00,0x00], b',' => [0x00,0x40,0x20,0x00,0x00],
        b'-' => [0x08,0x08,0x08,0x08,0x08], b'_' => [0x40,0x40,0x40,0x40,0x40],
        b'/' => [0x20,0x10,0x08,0x04,0x02], b'\\' => [0x02,0x04,0x08,0x10,0x20],
        b'=' => [0x14,0x14,0x14,0x14,0x14], b'+' => [0x08,0x08,0x3e,0x08,0x08],
        b'!' => [0x00,0x00,0x5f,0x00,0x00], b'?' => [0x02,0x01,0x51,0x09,0x06],
        b'#' => [0x14,0x7f,0x14,0x7f,0x14], b'\'' => [0x00,0x05,0x03,0x00,0x00],
        b'"' => [0x00,0x07,0x00,0x07,0x00], b'>' => [0x41,0x22,0x14,0x08,0x00],
        b'<' => [0x08,0x14,0x22,0x41,0x00], b'|' => [0x00,0x00,0x7f,0x00,0x00],
        b' ' => [0; 5], _ => [0x02,0x01,0x51,0x09,0x06],
    }
}

fn framebuffer_console_write_byte(byte: u8) -> bool {
    // SAFETY: the console is installed only after BootInfo and GOP geometry validation.
    unsafe {
        if let Some(console) = FRAMEBUFFER_CONSOLE.as_mut() {
            console.write_byte(byte);
            return true;
        }
    }
    false
}

fn replay_early_console_log() {
    // Interrupts are disabled during this single-threaded bootstrap; the buffer is
    // consumed only after the framebuffer has been validated and the renderer installed.
    unsafe {
        let len = EARLY_LOG_LEN;
        for index in 0..len {
            let byte = EARLY_LOG_BUFFER[index];
            let _ = framebuffer_console_write_byte(byte);
        }
        EARLY_LOG_LEN = 0;
    }
}

pub(super) struct Serial;

impl Serial {
    pub(super) fn init(&mut self) {
        unsafe {
            port_out(COM1 + 1, 0x00);
            port_out(COM1 + 3, 0x80);
            port_out(COM1, 0x03);
            port_out(COM1 + 1, 0x00);
            port_out(COM1 + 3, 0x03);
            port_out(COM1 + 2, 0xC7);
            port_out(COM1 + 4, 0x0B);
        }
    }
    fn write_byte(&mut self, byte: u8) {
        if !framebuffer_console_write_byte(byte) {
            // Keep serial output working even when GOP is missing, but replay its
            // earliest diagnostics if a valid framebuffer becomes available later.
            unsafe {
                if EARLY_LOG_LEN < EARLY_LOG_CAPACITY {
                    EARLY_LOG_BUFFER[EARLY_LOG_LEN] = byte;
                    EARLY_LOG_LEN += 1;
                }
            }
        }
        for _ in 0..100_000 {
            if unsafe { port_in(COM1 + 5) } & 0x20 != 0 {
                unsafe { port_out(COM1, byte) };
                return;
            }
            core::hint::spin_loop();
        }
    }
}
impl Write for Serial {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if byte == b'\n' { self.write_byte(b'\r'); }
            self.write_byte(byte);
        }
        Ok(())
    }
}
