use std::{
    fs,
    hash::{Hash, Hasher},
    path::Path,
};

pub fn valid_icon(path: &Path) -> bool {
    fs::read(path).is_ok_and(|bytes| {
        bytes.len() > 32
            && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
            && bytes.ends_with(b"IEND\xaeB\x60\x82")
    })
}
pub fn cached_icon(path: &str, dir: &Path, refresh: bool) -> Result<String, String> {
    // Shell parsers require Windows separators even though Rust accepts both.
    let normalized = path.replace('/', "\\");
    let path = normalized.as_str();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    "shell-image-factory-128-v2".hash(&mut hash);
    path.hash(&mut hash);
    if let Ok(meta) = fs::metadata(path) {
        meta.modified().ok().hash(&mut hash);
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let target = dir.join(format!("{:x}.png", hash.finish()));
    if refresh || !valid_icon(&target) {
        let (width, height, pixels) =
            high_resolution(path).or_else(|_| extract(path).map(|pixels| (32, 32, pixels)))?;
        // Encode before writing so concurrent requests never expose a partial PNG.
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
            writer
                .write_image_data(&pixels)
                .map_err(|e| e.to_string())?;
        }
        let tmp = target.with_extension(format!("{}.tmp", super::id()));
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        if let Err(e) = fs::rename(&tmp, &target) {
            let _ = fs::remove_file(&tmp);
            if !target.exists() {
                return Err(e.to_string());
            }
        }
    }
    Ok(target.to_string_lossy().to_string())
}

#[cfg(windows)]
fn high_resolution(path: &str) -> Result<(u32, u32, Vec<u8>), String> {
    use windows::Win32::{
        Foundation::SIZE,
        UI::Shell::{
            IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK,
            SIIGBF_ICONONLY,
        },
    };
    use windows_sys::Win32::{Graphics::Gdi::*, System::Com::*};
    unsafe {
        let com = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32);
        let result = (|| {
            let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
            let factory: IShellItemImageFactory = SHCreateItemFromParsingName(
                windows::core::PCWSTR(wide.as_ptr()),
                None::<&windows::Win32::System::Com::IBindCtx>,
            )
            .map_err(|e| e.to_string())?;
            let bitmap = factory
                .GetImage(
                    SIZE { cx: 128, cy: 128 },
                    SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK,
                )
                .map_err(|e| e.to_string())?;
            let decoded = (|| {
                let mut bm: BITMAP = std::mem::zeroed();
                if GetObjectW(
                    bitmap.0,
                    std::mem::size_of::<BITMAP>() as i32,
                    &mut bm as *mut _ as _,
                ) == 0
                {
                    return Err("无法读取高清图标位图".into());
                }
                let (width, height) = (bm.bmWidth as u32, bm.bmHeight.unsigned_abs());
                if width == 0 || height == 0 || width > 1024 || height > 1024 {
                    return Err("图标尺寸无效".into());
                }
                let mut info: BITMAPINFO = std::mem::zeroed();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = width as i32;
                info.bmiHeader.biHeight = -(height as i32);
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB;
                let mut pixels = vec![0u8; width as usize * height as usize * 4];
                let dc = CreateCompatibleDC(std::ptr::null_mut());
                if dc.is_null() {
                    return Err("无法读取高清图标像素".into());
                }
                let read = GetDIBits(
                    dc,
                    bitmap.0,
                    0,
                    height,
                    pixels.as_mut_ptr() as _,
                    &mut info,
                    DIB_RGB_COLORS,
                );
                DeleteDC(dc);
                if read == 0 {
                    return Err("无法读取高清图标像素".into());
                }
                for p in pixels.chunks_exact_mut(4) {
                    p.swap(0, 2);
                    // Shell image factories return premultiplied BGRA.
                    let a = p[3] as u32;
                    if a > 0 && a < 255 {
                        for c in &mut p[..3] {
                            *c = ((*c as u32 * 255) / a).min(255) as u8;
                        }
                    }
                }
                if !pixels.chunks_exact(4).any(|p| p[3] > 0) {
                    return Err("图标没有透明通道".into());
                }
                Ok((width, height, pixels))
            })();
            DeleteObject(bitmap.0);
            decoded
        })();
        if com >= 0 {
            CoUninitialize();
        }
        result
    }
}
#[cfg(not(windows))]
fn high_resolution(_path: &str) -> Result<(u32, u32, Vec<u8>), String> {
    Err("需要 Windows Shell".into())
}

#[cfg(windows)]
fn extract(path: &str) -> Result<Vec<u8>, String> {
    use std::{
        mem::{size_of, zeroed},
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Graphics::Gdi::*,
        System::Com::*,
        UI::{Shell::*, WindowsAndMessaging::*},
    };
    unsafe fn render(icon: HICON, background: u8) -> Result<Vec<u8>, String> {
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return Err("无法创建图标绘图上下文".into());
        }
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = 32;
        info.bmiHeader.biHeight = -32;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return Err("无法创建图标位图".into());
        }
        let old = SelectObject(dc, bitmap);
        std::ptr::write_bytes(bits as *mut u8, background, 32 * 32 * 4);
        let result = DrawIconEx(dc, 0, 0, icon, 32, 32, 0, null_mut(), DI_NORMAL);
        GdiFlush();
        let pixels = std::slice::from_raw_parts(bits as *const u8, 32 * 32 * 4).to_vec();
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        if result == 0 {
            return Err("无法绘制系统图标".into());
        }
        Ok(pixels)
    }
    unsafe {
        let com = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        let mut info: SHFILEINFOW = zeroed();
        let result = SHGetFileInfoW(
            path.as_ptr(),
            0,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        let pixels = if result == 0 || info.hIcon.is_null() {
            Err("无法读取系统图标".into())
        } else {
            // Rendering on both backgrounds preserves alpha for legacy icons too.
            let black = render(info.hIcon, 0);
            let white = render(info.hIcon, 255);
            DestroyIcon(info.hIcon);
            black.and_then(|black| {
                white.map(|white| {
                    let mut rgba = vec![0; 32 * 32 * 4];
                    for ((b, w), out) in black
                        .chunks_exact(4)
                        .zip(white.chunks_exact(4))
                        .zip(rgba.chunks_exact_mut(4))
                    {
                        let alpha = 255
                            - (0..3)
                                .map(|i| w[i].saturating_sub(b[i]))
                                .max()
                                .unwrap_or(255);
                        out[3] = alpha;
                        if alpha > 0 {
                            for i in 0..3 {
                                out[i] = ((b[2 - i] as u32 * 255) / alpha as u32).min(255) as u8;
                            }
                        }
                    }
                    rgba
                })
            })
        };
        if com >= 0 {
            CoUninitialize();
        }
        pixels
    }
}
#[cfg(not(windows))]
fn extract(_path: &str) -> Result<Vec<u8>, String> {
    Err("系统图标仅支持 Windows".into())
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(windows)]
    fn shell_icons_have_visible_pixels_and_transparency() {
        let path = format!("{}\\explorer.exe", std::env::var("WINDIR").unwrap());
        let rgba = super::extract(&path).unwrap();
        assert_eq!(rgba.len(), 32 * 32 * 4);
        assert!(rgba.chunks_exact(4).any(|p| p[3] > 0));
        assert!(rgba.chunks_exact(4).any(|p| p[3] == 0));
        let (width, height, rgba) = super::high_resolution(&path).unwrap();
        assert!(width >= 128 && height >= 128);
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        let dir = std::env::temp_dir().join(format!("qyflux-icons-{}", crate::id()));
        let file = super::cached_icon(&path, &dir, false).unwrap();
        std::fs::write(&file, b"truncated").unwrap();
        let repaired = super::cached_icon(&path, &dir, false).unwrap();
        assert!(super::valid_icon(std::path::Path::new(&repaired)));
        std::fs::remove_file(repaired).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
