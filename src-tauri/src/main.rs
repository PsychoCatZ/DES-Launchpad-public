#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default = "shortcut_mode")]
    mode: String,
    target: String,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    order: i32,
    #[serde(default)]
    args: Vec<String>,
}
fn shortcut_mode() -> String {
    "shortcut".into()
}
#[derive(Clone, Serialize, Deserialize)]
struct Config {
    apps: Vec<Entry>,
    #[serde(default)]
    groups: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    theme: String,
    browser: String,
    compact: bool,
    hide_after_launch: bool,
    hotkey: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            browser: "edge".into(),
            compact: false,
            hide_after_launch: true,
            hotkey: "Ctrl+Alt+D".into(),
        }
    }
}
struct Store {
    dir: PathBuf,
    config: Config,
    settings: Settings,
    warning: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    apps: Vec<Entry>,
    groups: Vec<String>,
    settings: Settings,
    config_path: String,
    autostart: bool,
    warning: Option<String>,
    icons: std::collections::HashMap<String, String>,
}

fn validate(config: &Config) -> Result<(), String> {
    let mut groups = HashSet::new();
    for group in &config.groups {
        if group.trim().is_empty() || group.len() > 160 || !groups.insert(group) {
            return Err("Названия разделов должны быть непустыми и уникальными.".into());
        }
    }
    if config.apps.len() > 1000 {
        return Err("Слишком много приложений (максимум 1000).".into());
    }
    let mut ids = HashSet::new();
    for entry in &config.apps {
        if entry.group.len() > 160 {
            return Err("Слишком длинное название раздела.".into());
        }
        if entry.id.is_empty()
            || !entry
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || !ids.insert(&entry.id)
        {
            return Err(
                "ID приложений должны быть уникальными: буквы a-z, цифры, дефис, подчёркивание."
                    .into(),
            );
        }
        if entry.name.trim().is_empty()
            || entry.target.trim().is_empty()
            || entry.target.contains('\0')
        {
            return Err("У приложения отсутствует имя или target.".into());
        }
        if !["web", "app"].contains(&entry.kind.as_str()) {
            return Err("Неизвестный тип приложения.".into());
        }
        if !["shortcut", "app-mode", "exe"].contains(&entry.mode.as_str()) {
            return Err("Неизвестный режим запуска.".into());
        }
        if entry.mode == "app-mode" {
            let u = url::Url::parse(&entry.target).map_err(|_| "Некорректный URL.")?;
            if !["http", "https"].contains(&u.scheme())
                || u.host_str().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
            {
                return Err("Разрешены только HTTP(S) URL без логина и пароля.".into());
            }
        }
    }
    Ok(())
}
fn validate_settings(s: &Settings) -> Result<(), String> {
    if !["system", "dark", "light"].contains(&s.theme.as_str())
        || !["edge", "chrome"].contains(&s.browser.as_str())
    {
        return Err("Некорректная тема или браузер.".into());
    }
    if !s.hotkey.is_empty() {
        let _: tauri_plugin_global_shortcut::Shortcut = s
            .hotkey
            .parse()
            .map_err(|_| "Неверная комбинация клавиш.")?;
    }
    Ok(())
}
fn read_config(path: &Path) -> Result<Config, String> {
    let bytes = fs::read(path).map_err(|_| "Не удалось прочитать apps.json.")?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "apps.json должен быть в UTF-8.")?
        .trim_start_matches('\u{feff}');
    let config: Config = serde_json::from_str(text)
        .map_err(|e| format!("Ошибка JSON: строка {}, столбец {}.", e.line(), e.column()))?;
    validate(&config)?;
    Ok(config)
}
fn log(dir: &Path, message: &str) {
    let path = dir.join("launchpad.log");
    if fs::metadata(&path)
        .map(|m| m.len() > 1_000_000)
        .unwrap_or(false)
    {
        let _ = fs::rename(&path, dir.join("launchpad.previous.log"));
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = writeln!(file, "{t} {message}");
    }
}
fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| "Ошибка сохранения JSON.")?;
    fs::write(&tmp, bytes).map_err(|_| "Не удалось записать настройки.")?;
    fs::rename(&tmp, path).map_err(|_| "Не удалось заменить файл настроек.".into())
}
fn apply_window_theme(app: &tauri::AppHandle, theme: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let value = match theme {
            "dark" => Some(tauri::Theme::Dark),
            "light" => Some(tauri::Theme::Light),
            _ => None,
        };
        let _ = w.set_theme(value);
    }
}
fn show(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
fn toggle(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false) {
            let _ = w.hide();
        } else {
            show(app)
        }
    }
}
fn register_hotkey(app: &tauri::AppHandle, key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    match app.global_shortcut().on_shortcut(key, |app,_,event| {if event.state()==ShortcutState::Pressed {toggle(app)}}) {
        Ok(())=>None,Err(_)=>Some(format!("Не удалось зарегистрировать {key}. Возможно, сочетание занято. Открытие через трей работает."))
    }
}
fn snapshot(app: &tauri::AppHandle, store: &Store) -> View {
    let mut icons = std::collections::HashMap::new();
    for e in &store.config.apps {
        // Only small PNG files inside this app's icons directory are exposed to the UI.
        let p = Path::new(&e.icon);
        if e.icon.starts_with("icons/")
            && p.components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
            && p.extension().is_some_and(|x| x == "png")
        {
            let full = store.dir.join(p);
            if fs::metadata(&full).is_ok_and(|m| m.len() <= 2_000_000) {
                if let Ok(bytes) = fs::read(full) {
                    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                        icons.insert(
                            e.id.clone(),
                            format!(
                                "data:image/png;base64,{}",
                                base64::engine::general_purpose::STANDARD.encode(bytes)
                            ),
                        );
                    }
                }
            }
        }
    }
    let mut apps = store.config.apps.clone();
    apps.sort_by_key(|e| e.order);
    View {
        apps,
        groups: store.config.groups.clone(),
        settings: store.settings.clone(),
        config_path: store.dir.join("apps.json").to_string_lossy().into(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        warning: store.warning.clone(),
        icons,
    }
}
#[tauri::command]
fn get_view(app: tauri::AppHandle, state: tauri::State<Mutex<Store>>) -> Result<View, String> {
    let s = state.lock().map_err(|_| "Хранилище занято.")?;
    Ok(snapshot(&app, &s))
}
#[tauri::command]
fn reload_config(app: tauri::AppHandle, state: tauri::State<Mutex<Store>>) -> Result<View, String> {
    let mut s = state.lock().map_err(|_| "Хранилище занято.")?;
    match read_config(&s.dir.join("apps.json")) {
        Ok(c) => {
            s.config = c;
            log(&s.dir, "config reloaded");
            Ok(snapshot(&app, &s))
        }
        Err(e) => {
            log(&s.dir, "config reload failed; previous config retained");
            Err(e)
        }
    }
}
#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<Mutex<Store>>,
    settings: Settings,
    autostart: bool,
) -> Result<View, String> {
    validate_settings(&settings)?;
    let mut s = state.lock().map_err(|_| "Хранилище занято.")?;
    let previous = app
        .autolaunch()
        .is_enabled()
        .map_err(|_| "Не удалось проверить автозапуск.")?;
    if autostart || previous != autostart {
        if autostart {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        }
        .map_err(|_| "Не удалось изменить автозапуск Windows.")?;
    }
    if let Err(e) = save_json(&s.dir.join("settings.json"), &settings) {
        if previous != autostart {
            let _ = if previous {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            };
        }
        return Err(e);
    }
    if s.settings.hotkey != settings.hotkey {
        let _ = app.global_shortcut().unregister_all();
        s.warning = register_hotkey(&app, &settings.hotkey);
    }
    apply_window_theme(&app, &settings.theme);
    s.settings = settings;
    log(&s.dir, "settings saved");
    Ok(snapshot(&app, &s))
}
fn shell_open(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        Err(format!(
            "Windows не выполнила запуск (код {}).",
            result as isize
        ))
    } else {
        Ok(())
    }
}
fn browser_path(browser: &str) -> Option<PathBuf> {
    let suffix = if browser == "chrome" {
        "Google/Chrome/Application/chrome.exe"
    } else {
        "Microsoft/Edge/Application/msedge.exe"
    };
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(|k| std::env::var_os(k))
        .map(|p| PathBuf::from(p).join(suffix))
        .find(|p| p.is_file())
}
fn shortcut_path(target: &str, data_dir: &Path) -> Result<PathBuf, String> {
    let path = Path::new(target);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    if path.starts_with("shortcuts")
        && path
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
    {
        return Ok(data_dir.join(path));
    }
    Err("Относительный ярлык должен находиться в папке shortcuts панели.".into())
}
fn launch_entry(entry: &Entry, settings: &Settings, data_dir: &Path) -> Result<(), String> {
    match entry.mode.as_str() {
        "shortcut" => {
            let path = shortcut_path(&entry.target, data_dir)?;
            if !path.is_file() {
                return Err("Ярлык не найден.".into());
            }
            if !path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("lnk"))
            {
                return Err("В режиме shortcut нужен файл .lnk.".into());
            }
            shell_open(&path)
        }
        "exe" => {
            let path = Path::new(&entry.target);
            if !path.is_file()
                || !path
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("exe"))
            {
                return Err("EXE не найден.".into());
            }
            Command::new(path)
                .args(&entry.args)
                .spawn()
                .map(|_| ())
                .map_err(|_| "Windows не запустила EXE.".into())
        }
        "app-mode" => {
            let browser =
                browser_path(&settings.browser).ok_or("Выбранный браузер не установлен.")?;
            Command::new(browser)
                .arg(format!("--app={}", entry.target))
                .spawn()
                .map(|_| ())
                .map_err(|_| "Не удалось запустить браузер.".into())
        }
        _ => Err("Неизвестный режим запуска.".into()),
    }
}
#[tauri::command]
fn launch(
    app: tauri::AppHandle,
    state: tauri::State<Mutex<Store>>,
    id: String,
) -> Result<(), String> {
    let s = state.lock().map_err(|_| "Хранилище занято.")?;
    let entry = s
        .config
        .apps
        .iter()
        .find(|e| e.id == id)
        .ok_or("Приложение не найдено в конфигурации.")?;
    match launch_entry(entry, &s.settings, &s.dir) {
        Ok(()) => {
            log(&s.dir, &format!("launch accepted id={}", entry.id));
            if s.settings.hide_after_launch {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        }
        Err(reason) => {
            log(
                &s.dir,
                &format!("launch failed id={} reason={reason}", entry.id),
            );
            Err(format!("Не удалось открыть {}. {reason}", entry.name))
        }
    }
}
#[tauri::command]
fn open_data_file(state: tauri::State<Mutex<Store>>, kind: String) -> Result<(), String> {
    let s = state.lock().map_err(|_| "Хранилище занято.")?;
    let p = match kind.as_str() {
        "config" => s.dir.join("apps.json"),
        "log" => s.dir.join("launchpad.log"),
        "folder" => s.dir.clone(),
        _ => return Err("Неизвестный файл.".into()),
    };
    shell_open(&p)
}

fn tray_icon() -> tauri::image::Image<'static> {
    // Constant identity colour; this launcher never displays health status.
    let size = 32u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - 16.0;
            let dy = y as f32 + 0.5 - 16.0;
            let alpha = ((14.0 - dx.hypot(dy) + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            rgba.extend_from_slice(&[0xA8, 0x55, 0xF7, alpha]);
        }
    }
    tauri::image::Image::new_owned(rgba, size, size)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app,_,_|show(app)))
        .plugin(tauri_plugin_autostart::Builder::new().args(["--background"]).build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![get_view,reload_config,save_settings,launch,open_data_file])
        .setup(|app| {
            let args:Vec<String>=std::env::args().collect();
            let dir=if let Some(i)=args.iter().position(|a|a=="--data-dir") {PathBuf::from(args.get(i+1).ok_or("--data-dir requires a path")?)}else{app.path().app_data_dir()?};
            fs::create_dir_all(dir.join("icons"))?;
            fs::create_dir_all(dir.join("shortcuts"))?;
            log(&dir,"application start");
            let defaults=if cfg!(debug_assertions) {PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../local")}else{app.path().resource_dir()?.join("defaults")};
            if !dir.join("apps.json").exists() {
                fs::copy(defaults.join("apps.json"),dir.join("apps.json"))?;
                for folder in ["icons", "shortcuts"] {
                    if let Ok(files)=fs::read_dir(defaults.join(folder)) {for file in files.flatten(){if file.path().is_file(){fs::copy(file.path(),dir.join(folder).join(file.file_name()))?;}}}
                }
            }
            let mut warnings=Vec::new();
            // Keep an existing opt-in bound to the binary the owner actually runs.
            // An absent or disabled startup entry is never enabled here.
            if app.autolaunch().is_enabled().unwrap_or(false) {
                if app.autolaunch().enable().is_err() {
                    warnings.push("Не удалось обновить путь уже включённого автозапуска.".into());
                }
            }
            let config=read_config(&dir.join("apps.json")).unwrap_or_else(|e|{warnings.push(e);Config{apps:vec![],groups:vec![]}});
            let settings=match fs::read(dir.join("settings.json")) {
                Ok(bytes)=>match serde_json::from_slice::<Settings>(&bytes) {Ok(s) if validate_settings(&s).is_ok()=>s,_=>{warnings.push("Не удалось загрузить настройки; применены временные значения по умолчанию.".into());Settings::default()}},
                Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Settings::default(),
                Err(_)=>{warnings.push("Файл настроек недоступен.".into());Settings::default()}
            };
            if let Some(w)=register_hotkey(app.handle(),&settings.hotkey){warnings.push(w)}
            for w in &warnings {log(&dir,w)}
            apply_window_theme(app.handle(), &settings.theme);
            app.manage(Mutex::new(Store{dir:dir.clone(),config,settings,warning:if warnings.is_empty(){None}else{Some(warnings.join(" "))}}));
            let open=MenuItem::with_id(app,"open","Открыть DES Launchpad",true,None::<&str>)?;
            let settings=MenuItem::with_id(app,"settings","Настройки",true,None::<&str>)?;
            let restart=MenuItem::with_id(app,"restart","Перезапустить",true,None::<&str>)?;
            let quit=MenuItem::with_id(app,"quit","Выход",true,None::<&str>)?;
            let menu=Menu::with_items(app,&[&open,&settings,&restart,&quit])?;
            if let Err(e)=TrayIconBuilder::new().icon(tray_icon()).tooltip("DES Launchpad").menu(&menu).show_menu_on_left_click(false)
                .on_menu_event(|app,event|match event.id.as_ref(){"open"=>show(app),"settings"=>{show(app);let _=app.emit("open-settings",());},"restart"=>app.restart(),"quit"=>app.exit(0),_=>{}})
                .on_tray_icon_event(|tray,event|{if matches!(event,TrayIconEvent::Click{button:MouseButton::Left,button_state:MouseButtonState::Up,..}) {toggle(tray.app_handle())}}).build(app) {
                    log(&dir,&format!("tray creation failed: {e}"));return Err(e.into())
                }
            log(&dir,"config loaded; tray ready");
            if args.iter().any(|a|a=="--show") {show(app.handle())}
            Ok(())
        })
        .on_window_event(|window,event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();let _=window.hide();}})
        .run(tauri::generate_context!())
        .expect("DES Launchpad: failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(target: &str, mode: &str) -> Config {
        Config {
            groups: vec![],
            apps: vec![Entry {
                id: "test".into(),
                name: "Тест".into(),
                kind: "web".into(),
                mode: mode.into(),
                target: target.into(),
                icon: String::new(),
                description: String::new(),
                order: 0,
                args: vec![],
                group: String::new(),
            }],
        }
    }
    #[test]
    fn rejects_command_urls_and_credentials() {
        for target in [
            "javascript:alert(1)",
            "file:///C:/test.exe",
            "https://user:password@example.com",
        ] {
            assert!(validate(&config(target, "app-mode")).is_err())
        }
    }
    #[test]
    fn accepts_url_as_single_argument() {
        assert!(validate(&config("https://example.com/path?a=1&b=2", "app-mode")).is_ok())
    }
    #[test]
    fn rejects_duplicate_ids() {
        let mut c = config("C:/test.lnk", "shortcut");
        c.apps.push(c.apps[0].clone());
        assert!(validate(&c).is_err())
    }
    #[test]
    fn malformed_reload_preserves_file() {
        let dir = std::env::temp_dir().join(format!("des-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("apps.json");
        fs::write(&path, "{oops}").unwrap();
        assert!(read_config(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{oops}");
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn missing_app_does_not_launch() {
        assert!(launch_entry(
            &config("Z:/nonexistent-des-test.lnk", "shortcut").apps[0],
            &Settings::default(),
            Path::new(".")
        )
        .is_err())
    }
    #[test]
    fn settings_round_trip() {
        let s = Settings {
            theme: "light".into(),
            ..Settings::default()
        };
        let restored: Settings = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(restored.theme, "light");
        assert!(validate_settings(&restored).is_ok());
    }
    #[test]
    fn legacy_config_without_groups_remains_compatible() {
        let cfg: Config = serde_json::from_str(
            r#"{"apps":[{"id":"old","name":"Старое","type":"web","target":"C:/old.lnk"}]}"#,
        )
        .unwrap();
        assert!(cfg.groups.is_empty());
        assert!(cfg.apps[0].group.is_empty());
        assert!(validate(&cfg).is_ok());
    }
    #[test]
    fn duplicate_group_names_are_rejected() {
        let mut cfg = config("C:/old.lnk", "shortcut");
        cfg.groups = vec!["Почта".into(), "Почта".into()];
        assert!(validate(&cfg).is_err());
    }
    #[test]
    fn owned_shortcut_survives_source_deletion() {
        let dir = std::env::temp_dir().join(format!("des-owned-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("shortcuts")).unwrap();
        let source = dir.join("desktop-original.lnk");
        fs::write(&source, b"shortcut bytes").unwrap();
        fs::copy(&source, dir.join("shortcuts/copied.lnk")).unwrap();
        fs::remove_file(&source).unwrap();
        let resolved = shortcut_path("shortcuts/copied.lnk", &dir).unwrap();
        assert_eq!(fs::read(&resolved).unwrap(), b"shortcut bytes");
        fs::remove_file(resolved).unwrap();
        fs::remove_dir(dir.join("shortcuts")).unwrap();
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn owned_shortcut_cannot_escape_data_directory() {
        assert!(shortcut_path("shortcuts/../../other.lnk", Path::new("C:/data")).is_err());
        assert!(shortcut_path("Desktop/app.lnk", Path::new("C:/data")).is_err());
    }
    #[test]
    fn saved_settings_replace_existing_file() {
        let dir = std::env::temp_dir().join(format!("des-settings-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        save_json(&path, &Settings::default()).unwrap();
        let changed = Settings {
            theme: "light".into(),
            compact: true,
            ..Settings::default()
        };
        save_json(&path, &changed).unwrap();
        let saved: Settings = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.theme, "light");
        assert!(saved.compact);
        assert!(!path.with_extension("tmp").exists());
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
