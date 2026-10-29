use anyhow::Result;
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, POINT, SIZE},
        Graphics::{
            Direct2D::{Common::*, *},
            DirectWrite::*,
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::*,
        },
        UI::WindowsAndMessaging::{ULW_ALPHA, UpdateLayeredWindow},
    },
    core::w,
};

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn has(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    pub fn d2d(self) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.x,
            top: self.y,
            right: self.x + self.w,
            bottom: self.y + self.h,
        }
    }
}
pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}
pub fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}

pub struct Painter {
    opacity: std::cell::Cell<f32>,
    group_opacity: std::cell::Cell<f32>,
    target: ID2D1DCRenderTarget,
    write: IDWriteFactory,
    normal: IDWriteTextFormat,
    centered: IDWriteTextFormat,
    label: IDWriteTextFormat,
    small: IDWriteTextFormat,
    title: IDWriteTextFormat,
    text_size: f32,
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    width: i32,
    height: i32,
    #[cfg(test)]
    pixels: *mut u8,
}
impl Painter {
    pub unsafe fn new() -> Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let target = factory.CreateDCRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_GDI_COMPATIBLE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            })?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let make = |size: f32| {
                write.CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("zh-CN"),
                )
            };
            let normal = make(14.0)?;
            let centered = make(14.0)?;
            centered.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            let label = make(11.0)?;
            label.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            label.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            label.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            let small = make(11.0)?;
            let title = make(16.0)?;
            let dc = CreateCompatibleDC(None);
            Ok(Self {
                group_opacity: std::cell::Cell::new(1.0),
                opacity: std::cell::Cell::new(1.0),
                target,
                write,
                normal,
                centered,
                label,
                small,
                title,
                text_size: 14.0,
                dc,
                bitmap: HBITMAP::default(),
                previous: HGDIOBJ::default(),
                width: 0,
                height: 0,
                #[cfg(test)]
                pixels: std::ptr::null_mut(),
            })
        }
    }
    pub unsafe fn begin(&mut self, width: i32, height: i32, scale: f32) -> Result<()> {
        unsafe {
            self.opacity.set(1.0);
            if self.width != width || self.height != height {
                if !self.bitmap.is_invalid() {
                    SelectObject(self.dc, self.previous);
                    let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
                }
                let info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        biHeight: -height,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut bits = std::ptr::null_mut();
                self.bitmap =
                    CreateDIBSection(Some(self.dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
                #[cfg(test)]
                {
                    self.pixels = bits.cast();
                }
                self.previous = SelectObject(self.dc, HGDIOBJ(self.bitmap.0));
                self.width = width;
                self.height = height;
            }
            self.target.BindDC(
                self.dc,
                &windows::Win32::Foundation::RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
            )?;
            self.target.SetTransform(&windows_numerics::Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: 0.0,
                M32: 0.0,
            });
            self.target.BeginDraw();
            self.target.Clear(Some(&color(0.0, 0.0, 0.0, 0.0)));
            Ok(())
        }
    }
    pub unsafe fn set_text_size(&mut self, size: u8) -> Result<()> {
        unsafe {
            let size = size.clamp(12, 18) as f32;
            if self.text_size == size {
                return Ok(());
            }
            let make = |n: f32| {
                self.write.CreateTextFormat(
                    w!("Segoe UI"),
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    n,
                    w!("zh-CN"),
                )
            };
            let normal = make(size)?;
            let centered = make(size)?;
            centered.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            // Compact hints keep their own size so larger todo text does not
            // clip shortcuts and secondary labels in narrow floating windows.
            let small = make(11.0)?;
            let title = make(size + 2.0)?;
            self.normal = normal;
            self.centered = centered;
            self.small = small;
            self.title = title;
            self.text_size = size;
            Ok(())
        }
    }
    pub unsafe fn end(&self, hwnd: HWND) -> Result<()> {
        unsafe {
            self.target.EndDraw(None, None)?;
            let mut bounds = windows::Win32::Foundation::RECT::default();
            windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut bounds)?;
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            UpdateLayeredWindow(
                hwnd,
                None,
                Some(&POINT {
                    x: bounds.left,
                    y: bounds.top,
                }),
                Some(&SIZE {
                    cx: self.width,
                    cy: self.height,
                }),
                Some(self.dc),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )?;
            Ok(())
        }
    }
    pub fn opacity(&self, value: f32) {
        self.opacity.set(value.clamp(0.0, 1.0));
    }
    pub fn group_opacity(&self, value: f32) {
        self.group_opacity.set(value.clamp(0.0, 1.0));
    }
    pub unsafe fn view(&self, scale_x: f32, scale_y: f32) {
        unsafe {
            self.target.SetTransform(&windows_numerics::Matrix3x2 {
                M11: scale_x,
                M22: scale_y,
                M12: 0.0,
                M21: 0.0,
                M31: 0.0,
                M32: 0.0,
            });
        }
    }
    fn tint(&self, mut c: D2D1_COLOR_F) -> D2D1_COLOR_F {
        c.a *= self.opacity.get() * self.group_opacity.get();
        c
    }
    pub unsafe fn clip(&self, r: Rect) {
        unsafe {
            self.target
                .PushAxisAlignedClip(&r.d2d(), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
        }
    }
    pub unsafe fn unclip(&self) {
        unsafe {
            self.target.PopAxisAlignedClip();
        }
    }
    pub unsafe fn present(&self, dc: HDC) -> Result<()> {
        unsafe {
            self.target.EndDraw(None, None)?;
            BitBlt(
                dc,
                0,
                0,
                self.width,
                self.height,
                Some(self.dc),
                0,
                0,
                SRCCOPY,
            )?;
            Ok(())
        }
    }
    #[cfg(test)]
    pub unsafe fn test_pixels(&self) -> Result<Vec<u8>> {
        unsafe {
            self.target.EndDraw(None, None)?;
            Ok(
                std::slice::from_raw_parts(self.pixels, (self.width * self.height * 4) as usize)
                    .to_vec(),
            )
        }
    }
    pub unsafe fn fill(&self, r: Rect, radius: f32, c: D2D1_COLOR_F) {
        unsafe {
            if let Ok(b) = self.target.CreateSolidColorBrush(&self.tint(c), None) {
                self.target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: r.d2d(),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &b,
                );
            }
        }
    }
    pub unsafe fn border(&self, r: Rect, radius: f32, c: D2D1_COLOR_F, width: f32) {
        unsafe {
            if let Ok(b) = self.target.CreateSolidColorBrush(&self.tint(c), None) {
                self.target.DrawRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: r.d2d(),
                        radiusX: radius,
                        radiusY: radius,
                    },
                    &b,
                    width,
                    None,
                );
            }
        }
    }
    pub unsafe fn line(&self, x: f32, y: f32, x2: f32, y2: f32, c: D2D1_COLOR_F, width: f32) {
        unsafe {
            if let Ok(b) = self.target.CreateSolidColorBrush(&self.tint(c), None) {
                self.target.DrawLine(
                    windows_numerics::Vector2 { X: x, Y: y },
                    windows_numerics::Vector2 { X: x2, Y: y2 },
                    &b,
                    width,
                    None,
                );
            }
        }
    }
    pub unsafe fn circle(&self, x: f32, y: f32, r: f32, c: D2D1_COLOR_F, filled: bool) {
        unsafe {
            if let Ok(b) = self.target.CreateSolidColorBrush(&self.tint(c), None) {
                let e = D2D1_ELLIPSE {
                    point: windows_numerics::Vector2 { X: x, Y: y },
                    radiusX: r,
                    radiusY: r,
                };
                if filled {
                    self.target.FillEllipse(&e, &b)
                } else {
                    self.target.DrawEllipse(&e, &b, 1.2, None)
                }
            }
        }
    }
    pub unsafe fn text(&self, text: &str, r: Rect, size: u8, c: D2D1_COLOR_F) {
        unsafe {
            let chars: Vec<u16> = text.encode_utf16().collect();
            let font = match size {
                0 => &self.small,
                2 => &self.title,
                3 => &self.centered,
                4 => &self.label,
                _ => &self.normal,
            };
            if let Ok(b) = self.target.CreateSolidColorBrush(&self.tint(c), None) {
                self.target.DrawText(
                    &chars,
                    font,
                    &r.d2d(),
                    &b,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
        }
    }
    pub unsafe fn text_height(&self, text: &str, width: f32) -> f32 {
        unsafe {
            let chars: Vec<u16> = text.encode_utf16().collect();
            if let Ok(layout) = self
                .write
                .CreateTextLayout(&chars, &self.normal, width, 500.0)
            {
                let mut m = DWRITE_TEXT_METRICS::default();
                if layout.GetMetrics(&mut m).is_ok() {
                    return m
                        .height
                        .clamp((self.text_size * 1.43).max(20.0), self.text_size * 4.3);
                }
            }
            20.0
        }
    }
    pub unsafe fn label_width(&self, text: &str) -> f32 {
        unsafe {
            let chars: Vec<u16> = text.encode_utf16().collect();
            if let Ok(layout) = self
                .write
                .CreateTextLayout(&chars, &self.label, 1000.0, 1000.0)
            {
                let mut metrics = DWRITE_TEXT_METRICS::default();
                if layout.GetMetrics(&mut metrics).is_ok() {
                    return metrics.width;
                }
            }
            40.0
        }
    }
}
impl Drop for Painter {
    fn drop(&mut self) {
        unsafe {
            if !self.bitmap.is_invalid() {
                SelectObject(self.dc, self.previous);
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            }
            let _ = DeleteDC(self.dc);
        }
    }
}
