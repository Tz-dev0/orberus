//! Claude Profiles in the menu bar: an icon, and a small window under it
//! that lists the profiles and starts Claude Code in one of them.
//!
//! This file only wires things to Tauri. What is shown and done lives in
//! `data.rs`, on top of the core shared with the `cprofile` CLI.

// A release build on Windows would otherwise open a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod data;

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, Rect, WebviewWindow, WindowEvent};

use claude_profiles::paths::Env;
use tauri_plugin_dialog::DialogExt;

const WINDOW: &str = "main";
const TRAY: &str = "main";
// Window size in points. The width matches tauri.conf.json.
const WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 80.0;
const MAX_HEIGHT: f64 = 680.0;

/// When the popover was last hidden because it lost focus. Shared state must
/// be safe to touch from several threads, hence the `Mutex`.
struct LastHidden(Mutex<Option<Instant>>);

/// True while the folder picker is open. An `AtomicBool` is a flag that
/// several threads can read and write without a lock.
struct Picking(AtomicBool);

/// When the window was last shown without a click (launch or reopen).
struct LastPresented(Mutex<Option<Instant>>);

/// True when `moment` holds a time less than `within` ago.
fn recent(moment: &Mutex<Option<Instant>>, within: Duration) -> bool {
    // A poisoned lock (another thread panicked while holding it) is read as
    // "no", which is the safe answer for both callers.
    moment
        .lock()
        .ok()
        .and_then(|guard| *guard)
        .is_some_and(|at| at.elapsed() < within)
}

/// Runs a core function and turns its error into a text. Tauri sends command
/// errors to the page as JSON, and `anyhow::Error` is not serializable.
/// `{error:#}` keeps the whole chain: "cannot read X: permission denied".
fn run<T>(action: impl FnOnce(&Env) -> Result<T>) -> Result<T, String> {
    Env::from_process()
        .and_then(|env| action(&env))
        .map_err(|error| format!("{error:#}"))
}

/// Asks macOS to open a file or a folder, as a double click would.
fn open(arguments: &[&std::ffi::OsStr]) -> Result<()> {
    let status = Command::new("open")
        .args(arguments)
        .status()
        .context("cannot run `open`")?;
    if !status.success() {
        bail!("`open` failed");
    }
    Ok(())
}

// Each `#[tauri::command]` function can be called from the page with
// `invoke("name", { arguments })`.

#[tauri::command]
fn overview() -> Result<data::Overview, String> {
    run(data::overview)
}

#[tauri::command]
fn set_default(name: String) -> Result<(), String> {
    run(|env| data::set_default(env, &name))
}

#[tauri::command]
fn add_profile(name: String, reuse_login: bool) -> Result<(), String> {
    run(|env| data::add_profile(env, &name, reuse_login))
}

// Both read whole folders, hence `async`: off the main thread.
#[tauri::command]
async fn conversations(name: String) -> Result<data::Conversations, String> {
    run(|env| data::conversations(env, &name))
}

#[tauri::command]
async fn trash_conversation(name: String, id: String) -> Result<(), String> {
    run(|env| data::trash_conversation(env, &name, &id))
}

#[tauri::command]
fn set_retention(name: String, days: u64) -> Result<(), String> {
    run(|env| data::set_retention(env, &name, days))
}

/// `async` so the measuring, which reads whole folders, runs off the main
/// thread and the window stays responsive meanwhile.
#[tauri::command]
async fn storage() -> Result<data::StorageOverview, String> {
    run(data::storage)
}

/// `async` for the same reason: it reads the conversations of the period.
#[tauri::command]
async fn usage() -> Result<data::UsageOverview, String> {
    run(data::usage)
}

#[tauri::command]
fn remove_profile(name: String, purge: bool) -> Result<(), String> {
    run(|env| data::remove_profile(env, &name, purge))
}

/// Shows the native folder picker and returns the chosen folder.
///
/// The popover normally hides as soon as it loses the focus, which also
/// puts this Dock-less app in the background: macOS then never brings the
/// picker to the screen. So while the picker is open, the popover stays.
///
/// `then_hide` says what becomes of the popover afterwards: hidden when the
/// choice ends its job (a terminal opens), brought back to the front when
/// the page still has a form to show.
fn pick_folder(app: &AppHandle, title: &str, start: PathBuf, then_hide: bool) -> Option<PathBuf> {
    app.state::<Picking>().0.store(true, Ordering::SeqCst);
    let picked = app
        .dialog()
        .file()
        .set_title(title)
        .set_directory(start)
        .blocking_pick_folder();
    if let Some(window) = app.get_webview_window(WINDOW) {
        if then_hide {
            let _ = window.hide();
        } else {
            // The picker took the focus: take it back before the popover
            // is allowed to hide on losing it again.
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    app.state::<Picking>().0.store(false, Ordering::SeqCst);
    picked.and_then(|folder| folder.into_path().ok())
}

/// Asks which folder a new rule is about, and returns what the form can
/// offer for it. `None` when the user cancels.
#[tauri::command]
async fn pick_rule_folder(app: AppHandle) -> Result<Option<data::RuleDraft>, String> {
    if app.state::<Picking>().0.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let env = Env::from_process().map_err(|error| format!("{error:#}"))?;
    let picked = pick_folder(&app, "Folder the rule is about", env.home.clone(), false);
    Ok(picked.map(|folder| data::rule_draft(&env, &folder)))
}

#[tauri::command]
fn add_rule(
    path: Option<String>,
    git_remote: Option<String>,
    profile: String,
    locked: bool,
) -> Result<(), String> {
    run(|env| data::add_rule(env, path, git_remote, &profile, locked))
}

/// The rule comes back as the page showed it, so that a list that changed
/// meanwhile never loses another rule.
#[tauri::command]
fn remove_rule(
    index: usize,
    path: Option<String>,
    git_remote: Option<String>,
    profile: String,
    locked: bool,
) -> Result<(), String> {
    let shown = claude_profiles::config::Rule {
        path,
        git_remote,
        profile,
        locked,
    };
    run(|env| data::remove_rule(env, index, &shown))
}

/// Asks for a project folder, then opens a terminal window running Claude
/// Code there with the given profile. Returns false when the user cancels.
///
/// `async` matters here: Tauri runs an async command off the main thread,
/// and the folder picker would freeze the app if it waited on the main one.
#[tauri::command]
async fn open_claude(app: AppHandle, name: String) -> Result<bool, String> {
    // A second click while the picker is open must not stack another one.
    if app.state::<Picking>().0.load(Ordering::SeqCst) {
        return Ok(false);
    }
    let env = Env::from_process().map_err(|error| format!("{error:#}"))?;
    // Start where this profile was last used, or in the home folder.
    let start = data::last_folder(&env, &name).unwrap_or_else(|| env.home.clone());
    let title = format!("Open Claude Code as {name} in");
    let Some(folder) = pick_folder(&app, &title, start, true) else {
        return Ok(false);
    };
    run(|env| {
        let launcher = data::write_launcher(env, &name, &folder)?;
        open(&[launcher.as_os_str()])
    })?;
    Ok(true)
}

/// Opens `config.toml` in the default text editor.
#[tauri::command]
fn open_config() -> Result<(), String> {
    run(|env| {
        let file = env.config_file();
        if !file.exists() {
            bail!("no config yet: create a profile with `cprofile add` in a terminal");
        }
        open(&["-t".as_ref(), file.as_os_str()])
    })
}

/// Gives the window the height of its content, so a short list does not
/// leave an empty panel. The page measures itself and calls this.
#[tauri::command]
fn fit(window: WebviewWindow, height: f64) {
    // Never trust a number coming from the page: keep it in a sane range.
    let height = height.clamp(MIN_HEIGHT, MAX_HEIGHT);
    let _ = window.set_size(LogicalSize::new(WIDTH, height));
}

#[tauri::command]
fn hide(window: WebviewWindow) {
    let _ = window.hide();
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

/// Puts the window right under the menu bar icon, centered on it.
fn place_under(window: &WebviewWindow, icon: Rect) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let position = icon.position.to_physical::<f64>(scale);
    let size = icon.size.to_physical::<f64>(scale);
    let Ok(own) = window.outer_size() else {
        return;
    };
    let width = f64::from(own.width);
    let mut x = position.x + size.width / 2.0 - width / 2.0;
    let y = position.y + size.height;

    // Near the right edge of the screen the window would stick out.
    if let Ok(Some(monitor)) = window
        .app_handle()
        .monitor_from_point(position.x, position.y)
    {
        let left = f64::from(monitor.position().x) + 8.0;
        let right = f64::from(monitor.position().x) + f64::from(monitor.size().width) - 8.0;
        x = x.min(right - width).max(left);
    }
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// A click on the icon shows the window, or hides it when it is open.
fn toggle(app: &AppHandle, icon: Rect) {
    let Some(window) = app.get_webview_window(WINDOW) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    // Clicking the icon while the window is open first takes the focus away,
    // which hides the window. Without this check the same click would then
    // open it again.
    if recent(&app.state::<LastHidden>().0, Duration::from_millis(300)) {
        return;
    }
    place_under(&window, icon);
    let _ = window.show();
    let _ = window.set_focus();
}

/// Shows the window without a click on the icon: at startup, and when the
/// app is opened again from Finder or Spotlight while it is running.
///
/// A menu bar app has no Dock icon and no window of its own, and on a screen
/// with a notch macOS hides the menu bar icons that do not fit. Without this,
/// opening the app could show nothing at all.
fn present(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW) else {
        return;
    };
    // `rect()` gives the icon's place in the menu bar, when macOS has one.
    let icon = app
        .tray_by_id(TRAY)
        .and_then(|tray| tray.rect().ok().flatten())
        // Right after launch macOS has not placed the icon yet and reports a
        // position outside every screen: only trust one that is on a screen.
        .filter(|icon| {
            let scale = window.scale_factor().unwrap_or(1.0);
            let at = icon.position.to_physical::<f64>(scale);
            matches!(app.monitor_from_point(at.x, at.y), Ok(Some(_)))
        });
    match icon {
        Some(icon) => place_under(&window, icon),
        None => place_top_right(&window),
    }
    if let Ok(mut last) = app.state::<LastPresented>().0.lock() {
        *last = Some(Instant::now());
    }
    let _ = window.show();
    let _ = window.set_focus();
}

/// Fallback when the icon's place is unknown: the top right corner of the
/// main screen, just under the menu bar.
fn place_top_right(window: &WebviewWindow) {
    let (Ok(Some(monitor)), Ok(own)) = (window.primary_monitor(), window.outer_size()) else {
        return;
    };
    let scale = monitor.scale_factor();
    let x = f64::from(monitor.position().x) + f64::from(monitor.size().width)
        - f64::from(own.width)
        - 12.0 * scale;
    let y = f64::from(monitor.position().y) + 40.0 * scale;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn main() {
    tauri::Builder::default()
        // The native folder picker.
        .plugin(tauri_plugin_dialog::init())
        .manage(LastHidden(Mutex::new(None)))
        .manage(Picking(AtomicBool::new(false)))
        .manage(LastPresented(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            overview,
            set_default,
            add_profile,
            remove_profile,
            set_retention,
            conversations,
            trash_conversation,
            storage,
            usage,
            pick_rule_folder,
            add_rule,
            remove_rule,
            open_claude,
            open_config,
            fit,
            hide,
            quit
        ])
        .setup(|app| {
            // A menu bar app: no icon in the Dock, no entry in Cmd-Tab.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Right click on the icon: a one-line menu to quit.
            let quit = MenuItem::with_id(app, "quit", "Quit Claude Profiles", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;
            // `include_bytes!` puts the PNG inside the binary at build time.
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;

            TrayIconBuilder::with_id(TRAY)
                .icon(icon)
                // A template icon is drawn by macOS in the menu bar color,
                // so it stays readable in light and dark mode.
                .icon_as_template(true)
                .tooltip("Claude Profiles")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        toggle(tray.app_handle(), rect);
                    }
                })
                .build(app)?;

            // Like any menu bar popover, it closes when you click elsewhere.
            let window = app
                .get_webview_window(WINDOW)
                .context("the main window is missing from tauri.conf.json")?;
            // The closure outlives `setup`, so it needs its own handle
            // (`clone` + `move`) rather than a borrow of `app`.
            let handle = app.handle().clone();
            window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    // The folder picker takes the focus: stay visible.
                    if handle.state::<Picking>().0.load(Ordering::SeqCst) {
                        return;
                    }
                    // While the app starts, the focus can go back to the app
                    // that launched it. The window must survive that, or
                    // opening the app would show nothing.
                    if recent(&handle.state::<LastPresented>().0, Duration::from_secs(2)) {
                        return;
                    }
                    if let Some(window) = handle.get_webview_window(WINDOW) {
                        let _ = window.hide();
                    }
                    if let Ok(mut last) = handle.state::<LastHidden>().0.lock() {
                        *last = Some(Instant::now());
                    }
                }
            });
            // Opening the app shows its window, so there is always something
            // to see. It waits a moment for macOS to place the menu bar icon,
            // on another thread so the app keeps starting meanwhile. Window
            // calls must then come back to the main thread.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(600));
                let on_main = handle.clone();
                let _ = handle.run_on_main_thread(move || present(&on_main));
            });
            Ok(())
        })
        // `build` + `run` instead of `run` alone, to receive app events.
        .build(tauri::generate_context!())
        .expect("cannot start Claude Profiles")
        .run(|app, event| {
            // Sent by macOS when the app is opened while already running.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                present(app);
            }
            // On other systems nothing is done with the event yet.
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
