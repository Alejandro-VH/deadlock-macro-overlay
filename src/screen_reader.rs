use std::ptr::null_mut;
use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::DataWriter;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    SRCCOPY,
};

/// Captures a screen rectangular region (ROI) into BGRA pixel bytes using Win32 GDI BitBlt.
pub fn capture_screen_roi(x: i32, y: i32, width: i32, height: i32) -> Option<Vec<u8>> {
    if width <= 0 || height <= 0 {
        return None;
    }

    unsafe {
        let hdc_screen = GetDC(HWND(null_mut()));
        if hdc_screen.0.is_null() {
            return None;
        }

        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem.0.is_null() {
            ReleaseDC(HWND(null_mut()), hdc_screen);
            return None;
        }

        let hbm = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbm.0.is_null() {
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(HWND(null_mut()), hdc_screen);
            return None;
        }

        let old_obj = SelectObject(hdc_mem, HGDIOBJ(hbm.0));

        let blt_res = BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, x, y, SRCCOPY);
        if blt_res.is_err() {
            SelectObject(hdc_mem, old_obj);
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(HWND(null_mut()), hdc_screen);
            return None;
        }

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height, // Top-down DIB
                biPlanes: 1,
                biBitCount: 32, // BGRA
                biCompression: BI_RGB.0,
                biSizeImage: (width * height * 4) as u32,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default(); 1],
        };

        let mut buffer = vec![0u8; (width * height * 4) as usize];
        let lines_copied = GetDIBits(
            hdc_mem,
            hbm,
            0,
            height as u32,
            Some(buffer.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old_obj);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(HWND(null_mut()), hdc_screen);

        if lines_copied == 0 {
            None
        } else {
            Some(buffer)
        }
    }
}

/// Parses recognized text string to find `MM:SS` match time format and returns total seconds.
pub fn parse_match_time(text: &str) -> Option<u32> {
    // Sanitize common OCR confusions: e.g. uppercase 'O' -> '0' and spaced colons
    let normalized = text.replace(" :", ":").replace(": ", ":");
    let sanitized: String = normalized
        .chars()
        .map(|c| match c {
            'O' | 'o' => '0',
            ';' => ':',
            other => other,
        })
        .collect();

    for token in sanitized.split_whitespace() {
        if let Some((min_str, sec_str)) = token.split_once(':') {
            let min_clean: String = min_str.chars().filter(|c| c.is_ascii_digit()).collect();
            let sec_clean: String = sec_str.chars().filter(|c| c.is_ascii_digit()).collect();

            if (1..=2).contains(&min_clean.len()) && sec_clean.len() == 2 {
                if let (Ok(minutes), Ok(seconds)) = (min_clean.parse::<u32>(), sec_clean.parse::<u32>()) {
                    if seconds < 60 && minutes < 120 {
                        return Some(minutes * 60 + seconds);
                    }
                }
            }
        }
    }

    None
}

/// Native Windows WinRT OCR reader.
pub struct ScreenOcr {
    engine: Option<OcrEngine>,
}

impl ScreenOcr {
    pub fn new() -> Self {
        let engine = OcrEngine::TryCreateFromUserProfileLanguages().ok();
        Self { engine }
    }

    /// Processes BGRA pixels through Windows Native OCR and returns extracted match seconds.
    pub fn read_time_from_bgra(&self, bgra_pixels: &[u8], width: i32, height: i32) -> Option<u32> {
        let engine = self.engine.as_ref()?;
        if width <= 0 || height <= 0 || bgra_pixels.len() != (width * height * 4) as usize {
            return None;
        }

        let writer = DataWriter::new().ok()?;
        writer.WriteBytes(bgra_pixels).ok()?;
        let ibuffer = writer.DetachBuffer().ok()?;

        let software_bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &ibuffer,
            BitmapPixelFormat::Bgra8,
            width,
            height,
        )
        .ok()?;

        let async_op = engine.RecognizeAsync(&software_bitmap).ok()?;
        let ocr_result = async_op.get().ok()?;

        let recognized_text = ocr_result.Text().ok()?.to_string();
        parse_match_time(&recognized_text)
    }
}

impl Default for ScreenOcr {
    fn default() -> Self {
        Self::new()
    }
}
