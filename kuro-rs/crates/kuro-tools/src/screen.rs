//! Primary-monitor capture through Win32 GDI, encoded as PNG.

use std::io::Cursor;

pub fn capture_primary_png() -> Option<Vec<u8>> {
    let (w, h, bgra) = capture_bgra()?;
    let rgba = bgra_to_rgba(&bgra);
    encode_png(w, h, &rgba)
}

pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || rgba.len() < (width as usize) * (height as usize) * 4 {
        return None;
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(Cursor::new(&mut out), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(rgba).ok()?;
    drop(writer);
    Some(out)
}

fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for px in bgra.chunks(4) {
        if px.len() < 4 {
            break;
        }
        rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    rgba
}

fn capture_bgra() -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        use windows_sys::Win32::Graphics::Gdi::{
            BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
            DeleteObject, GetDC, GetDIBits, ReleaseDC, SRCCOPY, SelectObject,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        if width <= 0 || height <= 0 {
            return None;
        }
        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            return None;
        }
        let memory = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        if memory.is_null() || bitmap.is_null() {
            ReleaseDC(std::ptr::null_mut(), screen);
            if !memory.is_null() {
                DeleteDC(memory);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap as _);
            }
            return None;
        }
        SelectObject(memory, bitmap as _);
        BitBlt(memory, 0, 0, width, height, screen, 0, 0, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [windows_sys::Win32::Graphics::Gdi::RGBQUAD {
                rgbBlue: 0,
                rgbGreen: 0,
                rgbRed: 0,
                rgbReserved: 0,
            }],
        };
        let mut pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        let rows =
            GetDIBits(memory, bitmap, 0, height as u32, pixels.as_mut_ptr() as *mut _, &mut info, DIB_RGB_COLORS);
        DeleteObject(bitmap as _);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), screen);
        if rows == 0 {
            return None;
        }
        Some((width as u32, height as u32, pixels))
    }
}
