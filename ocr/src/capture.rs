use crate::img::Bgr;
use windows_sys::Win32::Graphics::Gdi::*;

pub struct Grabber {
    screen: HDC,
    mem: HDC,
}

impl Grabber {
    pub fn new() -> Option<Self> {
        unsafe {
            windows_sys::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
                windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            );
            let screen = GetDC(std::ptr::null_mut());
            if screen.is_null() {
                return None;
            }
            let mem = CreateCompatibleDC(screen);
            Some(Grabber { screen, mem })
        }
    }

    pub fn grab(&self, x: i32, y: i32, w: i32, h: i32) -> Option<Bgr> {
        if w <= 0 || h <= 0 {
            return None;
        }
        unsafe {
            let bmp = CreateCompatibleBitmap(self.screen, w, h);
            if bmp.is_null() {
                return None;
            }
            let old = SelectObject(self.mem, bmp as _);
            let ok = BitBlt(self.mem, 0, 0, w, h, self.screen, x, y, SRCCOPY | CAPTUREBLT) != 0;
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = w;
            info.bmiHeader.biHeight = -h;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB as u32;
            let mut buf = vec![0u8; (w * h * 4) as usize];
            let lines = GetDIBits(self.mem, bmp, 0, h as u32, buf.as_mut_ptr() as _, &mut info, DIB_RGB_COLORS);
            SelectObject(self.mem, old);
            DeleteObject(bmp as _);
            if !ok || lines == 0 {
                return None;
            }
            let mut out = Bgr::new(w as usize, h as usize);
            for (d, s) in out.data.chunks_exact_mut(3).zip(buf.chunks_exact(4)) {
                d.copy_from_slice(&s[..3]);
            }
            Some(out)
        }
    }
}

impl Drop for Grabber {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.mem);
            ReleaseDC(std::ptr::null_mut(), self.screen);
        }
    }
}
