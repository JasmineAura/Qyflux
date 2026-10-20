#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
mod native_icons;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    name: String,
    path: String,
    kind: String,
    #[serde(default = "default_tab")]
    tab_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon_path: Option<String>,
}
fn default_tab() -> String {
    "default".into()
}
#[derive(Clone, Serialize, Deserialize)]
struct Tab {
    id: String,
    name: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Zone {
    id: String,
    name: String,
    entries: Vec<Entry>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde(default = "zone_color")]
    color: String,
    #[serde(default = "zone_opacity")]
    opacity: f64,
}
fn zone_color() -> String {
    "#171a25".into()
}
fn zone_opacity() -> f64 {
    0.22
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    accent: String,
    opacity: f64,
    material: String,
    background: Option<String>,
    glow: bool,
    motion: bool,
    shortcuts: Vec<Entry>,
    zones: Vec<Zone>,
    tabs: Vec<Tab>,
}
impl Default for Settings {
    fn default() -> Self {
        let root = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        Self {
            accent: "#a69aff".into(),
            opacity: 0.84,
            material: "acrylic".into(),
            background: None,
            glow: true,
            motion: true,
            zones: vec![],
            tabs: vec![Tab {
                id: default_tab(),
                name: "常用".into(),
            }],
            shortcuts: [
                ("explorer.exe", "文件资源管理器"),
                ("System32\\notepad.exe", "记事本"),
                ("System32\\calc.exe", "计算器"),
            ]
            .iter()
            .enumerate()
            .map(|(i, (path, name))| Entry {
                id: format!("builtin-{i}"),
                name: name.to_string(),
                path: format!("{root}\\{path}"),
                kind: "app".into(),
                tab_id: default_tab(),
                source_path: None,
                icon_path: None,
            })
            .collect(),
        }
    }
}
struct Store {
    settings: Mutex<Settings>,
    path: PathBuf,
}
fn id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string()
}
fn entry(path: String) -> Result<Entry, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("文件不存在：{path}"));
    }
    let name = p
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let kind = if p.is_dir() {
        "folder"
    } else if matches!(
        p.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "exe" | "lnk" | "url"
    ) {
        "app"
    } else {
        "file"
    };
    Ok(Entry {
        id: id(),
        name,
        path,
        kind: kind.into(),
        tab_id: default_tab(),
        source_path: None,
        icon_path: None,
    })
}
fn managed_shortcut(mut e: Entry, dir: &std::path::Path) -> Result<Entry, String> {
    let path = PathBuf::from(&e.path);
    let extension = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "lnk" || extension == "url" {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let target = dir.join(format!("{}.{}", e.id, extension));
        if path != target {
            fs::copy(&path, &target).map_err(|e| e.to_string())?;
        }
        e.source_path = Some(e.path);
        e.path = target.to_string_lossy().to_string();
    }
    Ok(e)
}
fn persist(store: &Store, settings: &Settings) -> Result<(), String> {
    let tmp = store.path.with_extension("tmp");
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(tmp, &store.path).map_err(|e| e.to_string())
}
fn update(
    app: &tauri::AppHandle,
    f: impl FnOnce(&mut Settings) -> Result<(), String>,
) -> Result<Settings, String> {
    let store = app.state::<Store>();
    let mut guard = store.settings.lock().map_err(|e| e.to_string())?;
    let mut next = guard.clone();
    f(&mut next)?;
    persist(&store, &next)?;
    *guard = next.clone();
    drop(guard);
    app.emit("settings-changed", &next)
        .map_err(|e| e.to_string())?;
    Ok(next)
}
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
fn show_toolbox_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("toolbox") {
        let _ = w.unminimize();
        let _ = w.show();
        clip_toolbox(&w);
        let _ = w.set_focus();
        let _ = w.emit("focus-search", ());
    }
}
#[tauri::command]
fn show_toolbox(app: tauri::AppHandle, tab_id: Option<String>) {
    show_toolbox_window(&app);
    if let Some(id) = tab_id {
        if let Some(w) = app.get_webview_window("toolbox") {
            let _ = w.emit("select-tab", id);
        }
    }
}
fn clip_toolbox(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    if let (Ok(hwnd), Ok(size), Ok(inner), Ok(outer)) = (
        window.hwnd(),
        window.inner_size(),
        window.inner_position(),
        window.outer_position(),
    ) {
        use windows_sys::Win32::Graphics::Gdi::{CreateEllipticRgn, DeleteObject, SetWindowRgn};
        // SetWindowRgn uses window coordinates, not client coordinates. Even an
        // undecorated window can have DPI-dependent invisible non-client borders.
        let x = inner.x - outer.x;
        let y = inner.y - outer.y;
        static LAST: Mutex<(isize, u32, u32, i32, i32)> = Mutex::new((0, 0, 0, 0, 0));
        let key = (hwnd.0 as isize, size.width, size.height, x, y);
        if LAST.lock().map(|last| *last == key).unwrap_or(false)
            || size.width == 0
            || size.height == 0
        {
            return;
        }
        unsafe {
            let region = CreateEllipticRgn(x, y, x + size.width as i32, y + size.height as i32);
            if !region.is_null() {
                if SetWindowRgn(hwnd.0, region, 1) == 0 {
                    DeleteObject(region);
                } else {
                    if let Ok(mut last) = LAST.lock() {
                        *last = key;
                    }
                }
            }
        }
    }
}
#[tauri::command]
fn show_control_panel(app: tauri::AppHandle) {
    show_main(&app);
}
fn material(w: &tauri::WebviewWindow, value: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        match value {
            // DWM compositor backdrop avoids legacy blur-behind repaint flashes.
            "acrylic" | "blur" => window_vibrancy::apply_acrylic(w, Some((18, 20, 30, 35))),
            _ => window_vibrancy::clear_acrylic(w),
        }
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn effective_material(settings: &Settings) -> &str {
    if settings.background.is_some() {
        "clear"
    } else {
        &settings.material
    }
}
fn apply_materials(app: &tauri::AppHandle, settings: &Settings) -> Result<(), String> {
    for w in app
        .webview_windows()
        .values()
        .filter(|w| !w.label().starts_with("zone-"))
    {
        material(
            w,
            if w.label() == "toolbox" {
                "clear"
            } else {
                effective_material(settings)
            },
        )?;
    }
    Ok(())
}
fn create_zone_window(app: &tauri::AppHandle, zone: &Zone, _effect: &str) -> Result<(), String> {
    let label = format!("zone-{}", zone.id);
    if let Some(w) = app.get_webview_window(&label) {
        w.show().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let w = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::App(format!("index.html?zone={}", zone.id).into()),
    )
    .title(&zone.name)
    .inner_size(zone.width, zone.height)
    .min_inner_size(240., 180.)
    .position(zone.x, zone.y)
    .transparent(true)
    .background_color(tauri::window::Color(0, 0, 0, 0))
    .decorations(false)
    .skip_taskbar(true)
    .shadow(false)
    .build()
    .map_err(|e| e.to_string())?;
    // Desktop spaces intentionally have no native backdrop or opaque WebView fill.
    let _ = w;
    Ok(())
}
#[tauri::command]
fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    Ok(app
        .state::<Store>()
        .settings
        .lock()
        .map_err(|e| e.to_string())?
        .clone())
}
#[tauri::command]
fn save_appearance(
    app: tauri::AppHandle,
    accent: String,
    opacity: f64,
    effect: String,
    glow: bool,
    motion: bool,
) -> Result<Settings, String> {
    if accent.len() != 7
        || !accent.starts_with('#')
        || !accent[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("主题色格式无效".into());
    }
    if !opacity.is_finite() || !(0.15..=1.).contains(&opacity) {
        return Err("不透明度范围为 15%—100%".into());
    }
    if !["acrylic", "blur", "clear"].contains(&effect.as_str()) {
        return Err("未知材质".into());
    }
    let old = get_settings(app.clone())?;
    let mut next = old.clone();
    next.material = effect.clone();
    if effective_material(&old) != effective_material(&next) {
        apply_materials(&app, &next)?;
    }
    update(&app, |s| {
        s.accent = accent;
        s.opacity = opacity;
        s.material = effect;
        s.glow = glow;
        s.motion = motion;
        Ok(())
    })
}
#[tauri::command]
async fn pick_background(app: tauri::AppHandle) -> Result<Settings, String> {
    let file = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(path) = file {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("backgrounds");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let target = dir.join(format!(
            "{}.{}",
            id(),
            path.extension().unwrap_or_default().to_string_lossy()
        ));
        fs::copy(path, &target).map_err(|e| e.to_string())?;
        let saved = update(&app, |s| {
            s.background = Some(target.to_string_lossy().to_string());
            Ok(())
        })?;
        apply_materials(&app, &saved)?;
        return Ok(saved);
    }
    get_settings(app)
}
#[tauri::command]
fn clear_background(app: tauri::AppHandle) -> Result<Settings, String> {
    let saved = update(&app, |s| {
        s.background = None;
        Ok(())
    })?;
    apply_materials(&app, &saved)?;
    Ok(saved)
}
#[tauri::command]
fn autostart_status(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}
#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mgr = app.autolaunch();
    if enabled { mgr.enable() } else { mgr.disable() }.map_err(|e| e.to_string())
}
#[tauri::command]
fn add_paths(
    app: tauri::AppHandle,
    zone_id: Option<String>,
    paths: Vec<String>,
    tab_id: Option<String>,
) -> Result<Settings, String> {
    let mut entries: Vec<Entry> = paths.into_iter().map(entry).collect::<Result<_, _>>()?;
    let shortcut_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("shortcuts");
    update(&app, |s| {
        if zone_id.is_none() {
            let tab = tab_id.unwrap_or_else(default_tab);
            if !s.tabs.iter().any(|t| t.id == tab) {
                return Err("标签不存在".into());
            }
            for e in &mut entries {
                e.tab_id = tab.clone();
            }
        }
        let list = if let Some(zone_id) = zone_id {
            &mut s
                .zones
                .iter_mut()
                .find(|z| z.id == zone_id)
                .ok_or("区域不存在")?
                .entries
        } else {
            &mut s.shortcuts
        };
        for e in entries {
            if !list.iter().any(|old| {
                old.source_path
                    .as_deref()
                    .unwrap_or(&old.path)
                    .eq_ignore_ascii_case(&e.path)
                    && old.tab_id == e.tab_id
            }) {
                list.push(managed_shortcut(e, &shortcut_dir)?);
            }
        }
        Ok(())
    })
}
#[tauri::command]
async fn pick_entries(
    app: tauri::AppHandle,
    zone_id: Option<String>,
    folder: bool,
    tab_id: Option<String>,
) -> Result<Settings, String> {
    let paths = tauri::async_runtime::spawn_blocking(move || {
        let picker = rfd::FileDialog::new();
        if folder {
            picker.pick_folders()
        } else {
            picker.pick_files()
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(paths) = paths {
        add_paths(
            app,
            zone_id,
            paths
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            tab_id,
        )
    } else {
        get_settings(app)
    }
}
#[tauri::command]
fn remove_entry(
    app: tauri::AppHandle,
    zone_id: Option<String>,
    entry_id: String,
) -> Result<Settings, String> {
    update(&app, |s| {
        let list = if let Some(zone_id) = zone_id {
            &mut s
                .zones
                .iter_mut()
                .find(|z| z.id == zone_id)
                .ok_or("区域不存在")?
                .entries
        } else {
            &mut s.shortcuts
        };
        list.retain(|e| e.id != entry_id);
        Ok(())
    })
}
#[tauri::command]
async fn launch_entry(app: tauri::AppHandle, entry_id: String) -> Result<(), String> {
    let s = get_settings(app.clone())?;
    let e = s
        .shortcuts
        .iter()
        .chain(s.zones.iter().flat_map(|z| &z.entries))
        .find(|e| e.id == entry_id)
        .ok_or("找不到快捷方式")?;
    let path = e.path.clone();
    if !PathBuf::from(&path).exists() {
        return Err("原文件已移动或删除，请重新添加".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        open::that_detached(path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn create_zone(app: tauri::AppHandle, name: String) -> Result<Settings, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 32 {
        return Err("区域名称需为 1—32 个字符".into());
    }
    let s = get_settings(app.clone())?;
    let zone = Zone {
        id: id(),
        name: name.into(),
        entries: vec![],
        x: 80. + (s.zones.len() % 6) as f64 * 38.,
        y: 90. + (s.zones.len() % 6) as f64 * 38.,
        width: 340.,
        height: 360.,
        color: zone_color(),
        opacity: zone_opacity(),
    };
    // Persist before starting the WebView; its first snapshot must contain the zone.
    let saved = update(&app, |s| {
        s.zones.push(zone.clone());
        Ok(())
    })?;
    if let Err(e) = create_zone_window(&app, &zone, &s.material) {
        let _ = update(&app, |s| {
            s.zones.retain(|z| z.id != zone.id);
            Ok(())
        });
        return Err(e);
    }
    Ok(saved)
}
#[tauri::command]
fn save_zone_appearance(
    app: tauri::AppHandle,
    zone_id: String,
    color: String,
    opacity: f64,
) -> Result<Settings, String> {
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("区域底色无效".into());
    }
    if !opacity.is_finite() || !(0.0..=0.85).contains(&opacity) {
        return Err("区域不透明度需在 0%—85% 之间".into());
    }
    update(&app, |s| {
        let zone = s
            .zones
            .iter_mut()
            .find(|z| z.id == zone_id)
            .ok_or("区域不存在")?;
        zone.color = color;
        zone.opacity = opacity;
        Ok(())
    })
}
#[tauri::command]
fn remove_zone(app: tauri::AppHandle, zone_id: String) -> Result<Settings, String> {
    let result = update(&app, |s| {
        s.zones.retain(|z| z.id != zone_id);
        Ok(())
    })?;
    if let Some(w) = app.get_webview_window(&format!("zone-{zone_id}")) {
        w.close().map_err(|e| e.to_string())?;
    }
    Ok(result)
}
#[tauri::command]
async fn show_zone(app: tauri::AppHandle, zone_id: String) -> Result<(), String> {
    let s = get_settings(app.clone())?;
    let z = s
        .zones
        .iter()
        .find(|z| z.id == zone_id)
        .ok_or("区域不存在")?;
    create_zone_window(&app, z, &s.material)?;
    if let Some(w) = app.get_webview_window(&format!("zone-{zone_id}")) {
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn save_zone_bounds(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    let zid = window
        .label()
        .strip_prefix("zone-")
        .ok_or("仅限桌面区域")?
        .to_string();
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let pos = window
        .outer_position()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    let size = window
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    update(&app, |s| {
        if let Some(z) = s.zones.iter_mut().find(|z| z.id == zid) {
            z.x = pos.x;
            z.y = pos.y;
            z.width = size.width;
            z.height = size.height;
        }
        Ok(())
    })?;
    Ok(())
}
#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}
#[tauri::command]
fn shortcut_error(app: tauri::AppHandle) -> Option<String> {
    app.try_state::<ShortcutError>().map(|e| e.0.clone())
}

fn tab_name(name: String) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 24 {
        return Err("标签名称需为 1—24 个字符".into());
    }
    Ok(name.into())
}
#[tauri::command]
fn create_tab(app: tauri::AppHandle, name: String) -> Result<Settings, String> {
    let name = tab_name(name)?;
    update(&app, |s| {
        if s.tabs.iter().any(|t| t.name == name) {
            return Err("标签名称已存在".into());
        }
        s.tabs.push(Tab { id: id(), name });
        Ok(())
    })
}
#[tauri::command]
fn rename_tab(app: tauri::AppHandle, tab_id: String, name: String) -> Result<Settings, String> {
    let name = tab_name(name)?;
    update(&app, |s| {
        if s.tabs.iter().any(|t| t.id != tab_id && t.name == name) {
            return Err("标签名称已存在".into());
        }
        s.tabs
            .iter_mut()
            .find(|t| t.id == tab_id)
            .ok_or("标签不存在")?
            .name = name;
        Ok(())
    })
}
#[tauri::command]
fn remove_tab(app: tauri::AppHandle, tab_id: String) -> Result<Settings, String> {
    if tab_id == default_tab() {
        return Err("默认标签不能删除".into());
    }
    update(&app, |s| {
        s.tabs.retain(|t| t.id != tab_id);
        // Preserve every reference when removing a category.
        for e in &mut s.shortcuts {
            if e.tab_id == tab_id {
                e.tab_id = default_tab();
            }
        }
        Ok(())
    })
}
#[tauri::command]
fn move_entry(app: tauri::AppHandle, entry_id: String, tab_id: String) -> Result<Settings, String> {
    update(&app, |s| {
        if !s.tabs.iter().any(|t| t.id == tab_id) {
            return Err("标签不存在".into());
        }
        s.shortcuts
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or("快捷方式不存在")?
            .tab_id = tab_id;
        Ok(())
    })
}
#[tauri::command]
async fn get_entry_icon(
    app: tauri::AppHandle,
    entry_id: String,
    refresh: Option<bool>,
) -> Result<String, String> {
    let s = get_settings(app.clone())?;
    let entry = s
        .shortcuts
        .iter()
        .chain(s.zones.iter().flat_map(|z| &z.entries))
        .find(|e| e.id == entry_id)
        .ok_or("快捷方式不存在")?;
    if let Some(icon) = &entry.icon_path {
        if !refresh.unwrap_or(false) && native_icons::valid_icon(&PathBuf::from(icon)) {
            return Ok(icon.clone());
        }
    }
    let path = entry.path.clone();
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("icons");
    let expected_path = path.clone();
    let icon = tauri::async_runtime::spawn_blocking(move || {
        native_icons::cached_icon(&path, &dir, refresh.unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())??;
    // Cache metadata without broadcasting a full UI rebuild for each loaded icon.
    let store = app.state::<Store>();
    let mut guard = store.settings.lock().map_err(|e| e.to_string())?;
    let mut next = guard.clone();
    if let Some(e) = next
        .shortcuts
        .iter_mut()
        .chain(next.zones.iter_mut().flat_map(|z| &mut z.entries))
        .find(|e| e.id == entry_id && e.path == expected_path)
    {
        e.icon_path = Some(icon.clone());
        persist(&store, &next)?;
        *guard = next;
    }
    Ok(icon)
}
#[tauri::command]
async fn rebind_entry(app: tauri::AppHandle, entry_id: String) -> Result<Settings, String> {
    let settings = get_settings(app.clone())?;
    let old = settings
        .shortcuts
        .iter()
        .chain(settings.zones.iter().flat_map(|z| &z.entries))
        .find(|e| e.id == entry_id)
        .ok_or("快捷方式不存在")?
        .clone();
    let folder = old.kind == "folder";
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let picker = rfd::FileDialog::new().set_title("重新绑定文件，保留区域和名称");
        if folder {
            picker.pick_folder()
        } else {
            picker.pick_file()
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = picked else {
        return get_settings(app);
    };
    let mut next = entry(path.to_string_lossy().to_string())?;
    next.id = old.id.clone();
    next.name = old.name;
    next.tab_id = old.tab_id;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("shortcuts");
    let next = managed_shortcut(next, &dir)?;
    update(&app, |s| {
        let slot = s
            .shortcuts
            .iter_mut()
            .chain(s.zones.iter_mut().flat_map(|z| &mut z.entries))
            .find(|e| e.id == entry_id)
            .ok_or("快捷方式不存在")?;
        *slot = next;
        Ok(())
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Some(w) = app.get_webview_window("toolbox") {
                            if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) {
                                let _ = w.hide();
                            } else {
                                show_toolbox_window(app);
                            }
                        }
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_appearance,
            pick_background,
            clear_background,
            autostart_status,
            set_autostart,
            add_paths,
            pick_entries,
            remove_entry,
            launch_entry,
            create_zone,
            remove_zone,
            show_zone,
            save_zone_bounds,
            exit_app,
            shortcut_error,
            show_toolbox,
            show_control_panel,
            create_tab,
            rename_tab,
            remove_tab,
            move_entry,
            get_entry_icon,
            rebind_entry,
            save_zone_appearance
        ])
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            fs::create_dir_all(&dir)?;
            let path = dir.join("settings.json");
            let settings: Settings = if path.exists() {
                match serde_json::from_slice(&fs::read(&path)?) {
                    Ok(s) => s,
                    Err(e) => {
                        fs::copy(&path, dir.join(format!("settings-corrupt-{}.json", id())))?;
                        eprintln!("配置无法读取，已备份：{e}");
                        Settings::default()
                    }
                }
            } else {
                Settings::default()
            };
            app.manage(Store {
                settings: Mutex::new(settings.clone()),
                path,
            });
            let toolbox_diameter = app
                .primary_monitor()?
                .map(|monitor| {
                    let size = monitor.work_area().size;
                    (size.width.min(size.height) as f64 / monitor.scale_factor() - 28.)
                        .clamp(460., 780.)
                })
                .unwrap_or(700.);
            let toolbox =
                WebviewWindowBuilder::new(app, "toolbox", WebviewUrl::App("toolbox.html".into()))
                    .title("Qyflux 工具箱")
                    .inner_size(toolbox_diameter, toolbox_diameter)
                    .resizable(false)
                    .center()
                    .decorations(false)
                    .shadow(false)
                    .transparent(true)
                    .background_color(tauri::window::Color(0, 0, 0, 255))
                    .skip_taskbar(true)
                    .always_on_top(true)
                    .visible(false)
                    .build()?;
            clip_toolbox(&toolbox);
            if let Err(e) = material(&toolbox, "clear") {
                eprintln!("工具箱材质不可用：{e}");
            }
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
            let show = MenuItem::with_id(app, "show", "工具箱    Alt+Space", true, None::<&str>)?;
            let control = MenuItem::with_id(app, "control", "Qyflux 控制面板", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出 Qyflux", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &control, &quit])?;
            let mut pixels = vec![0u8; 32 * 32 * 4];
            for y in 0..32 {
                for x in 0..32 {
                    let d = ((x as f32 - 15.5).powi(2) + (y as f32 - 15.5).powi(2)).sqrt();
                    if (8.0..12.0).contains(&d) || (x > 18 && x < 27 && y > 19 && y < 24) {
                        let i = (y * 32 + x) * 4;
                        pixels[i..i + 4].copy_from_slice(&[174, 156, 255, 255]);
                    }
                }
            }
            TrayIconBuilder::new()
                .icon(tauri::image::Image::new_owned(pixels, 32, 32))
                .tooltip("Qyflux · Alt+Space")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_toolbox_window(app),
                    "control" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_toolbox_window(tray.app_handle());
                    }
                })
                .build(app)?;
            if let Err(e) = app.global_shortcut().register("Alt+Space") {
                app.manage(ShortcutError(e.to_string()));
            }
            if let Some(w) = app.get_webview_window("main") {
                if let Err(e) = material(&w, effective_material(&settings)) {
                    eprintln!("材质不可用：{e}");
                }
            }
            for z in &settings.zones {
                if let Err(e) = create_zone_window(app.handle(), z, &settings.material) {
                    eprintln!("无法恢复区域：{e}");
                }
            }
            if !std::env::args().any(|a| a == "--background") {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            if w.label() == "toolbox"
                && matches!(
                    e,
                    tauri::WindowEvent::Resized(_)
                        | tauri::WindowEvent::Moved(_)
                        | tauri::WindowEvent::ScaleFactorChanged { .. }
                )
            {
                if let Some(webview) = w.app_handle().get_webview_window("toolbox") {
                    clip_toolbox(&webview);
                }
            }
            if w.label() == "main" || w.label() == "toolbox" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                    api.prevent_close();
                    let _ = w.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Qyflux 启动失败");
}
struct ShortcutError(String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_references_preserve_originals_and_reject_missing_paths() {
        let dir = std::env::temp_dir().join(format!("qyflux-test-{}", id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("灵感.txt");
        fs::write(&path, "keep me").unwrap();
        let e = entry(path.to_string_lossy().to_string()).unwrap();
        assert_eq!(e.name, "灵感");
        assert_eq!(e.kind, "file");
        drop(e);
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep me");
        assert_eq!(
            entry(dir.to_string_lossy().to_string()).unwrap().kind,
            "folder"
        );
        assert!(entry(dir.join("missing.exe").to_string_lossy().to_string()).is_err());
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&dir).unwrap();
    }

    #[test]
    fn saving_replaces_existing_settings_and_restores_zones() {
        let dir = std::env::temp_dir().join(format!("qyflux-test-{}", id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let store = Store {
            path: path.clone(),
            settings: Mutex::new(Settings::default()),
        };
        persist(&store, &Settings::default()).unwrap();
        let mut next = Settings::default();
        next.accent = "#8fe3cf".into();
        next.zones.push(Zone {
            id: id(),
            name: "灵感".into(),
            entries: vec![],
            x: -120.,
            y: 80.,
            width: 340.,
            height: 360.,
            color: zone_color(),
            opacity: zone_opacity(),
        });
        persist(&store, &next).unwrap();
        let loaded: Settings = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(loaded.accent, "#8fe3cf");
        assert_eq!(loaded.zones[0].name, "灵感");
        assert_eq!(loaded.zones[0].x, -120.);
        assert!(!path.with_extension("tmp").exists());
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn older_configs_receive_new_defaults() {
        let settings: Settings = serde_json::from_str(r##"{"accent":"#8fe3cf"}"##).unwrap();
        assert_eq!(settings.accent, "#8fe3cf");
        assert!(settings.glow && settings.motion);
        assert!(settings.zones.is_empty());
        assert_eq!(settings.tabs[0].id, "default");
        let old: Entry = serde_json::from_str(
            r#"{"id":"legacy","name":"旧快捷方式","path":"C:\\test.exe","kind":"app"}"#,
        )
        .unwrap();
        assert_eq!(old.tab_id, "default");
    }

    #[test]
    fn labels_reject_empty_or_overlong_names() {
        assert!(tab_name("  ".into()).is_err());
        assert!(tab_name("a".repeat(25)).is_err());
        assert_eq!(tab_name("  创作  ".into()).unwrap(), "创作");
    }
    #[test]
    fn managed_shortcut_survives_removal_of_desktop_original() {
        let dir = std::env::temp_dir().join(format!("qyflux-test-{}", id()));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Desktop.lnk");
        fs::write(&source, b"test shortcut").unwrap();
        let saved = managed_shortcut(
            entry(source.to_string_lossy().to_string()).unwrap(),
            &dir.join("owned"),
        )
        .unwrap();
        fs::remove_file(&source).unwrap();
        assert_eq!(fs::read(&saved.path).unwrap(), b"test shortcut");
        assert_eq!(saved.source_path.unwrap(), source.to_string_lossy());
        fs::remove_file(&saved.path).unwrap();
        fs::remove_dir(dir.join("owned")).unwrap();
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn wallpaper_disables_native_glass_without_losing_preference() {
        let mut s = Settings::default();
        s.material = "blur".into();
        assert_eq!(effective_material(&s), "blur");
        s.background = Some("wallpaper.jpg".into());
        assert_eq!(effective_material(&s), "clear");
        assert_eq!(s.material, "blur");
        s.background = None;
        assert_eq!(effective_material(&s), "blur");
    }
}
