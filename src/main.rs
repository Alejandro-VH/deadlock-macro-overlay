#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deadlock_overlay::app::DeadlockApp;
use deadlock_overlay::config::OverlayConfig;
use deadlock_overlay::screen_reader::{capture_screen_roi, ScreenOcr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::core::w;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
use windows::Win32::Graphics::Gdi::{GetDC, GetDeviceCaps, ReleaseDC, LOGPIXELSX};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_TAB};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowW, GetForegroundWindow, GetSystemMetrics, GetWindowLongPtrW,
    GetWindowTextW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST,
    SM_CXSCREEN, SM_CYSCREEN, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, WS_EX_LAYERED, WS_EX_TRANSPARENT, WS_MAXIMIZEBOX, WS_THICKFRAME,
};

const WINDOW_TITLE: &str = "DeadlockMacroOverlay";

/// Adjusts the Win32 WS_EX_TRANSPARENT extended style to toggle mouse click-through with SWP_FRAMECHANGED.
fn apply_click_through(hwnd: HWND, click_through: bool) {
    if hwnd.0.is_null() {
        return;
    }
    unsafe {
        let current_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_style = if click_through {
            current_style | (WS_EX_TRANSPARENT.0 as isize) | (WS_EX_LAYERED.0 as isize)
        } else {
            (current_style & !(WS_EX_TRANSPARENT.0 as isize)) | (WS_EX_LAYERED.0 as isize)
        };
        if current_style != new_style {
            let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

/// Strips WS_MAXIMIZEBOX and WS_THICKFRAME to completely disable Windows Aero Snap / auto-scaling.
fn strip_snap_styles(hwnd: HWND) {
    if hwnd.0.is_null() {
        return;
    }
    unsafe {
        let current_style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let stripped_style = current_style & !(WS_MAXIMIZEBOX.0 as isize) & !(WS_THICKFRAME.0 as isize);
        if stripped_style != current_style {
            let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, stripped_style);
            let _ = SetWindowPos(
                hwnd,
                HWND(std::ptr::null_mut()),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }
}

/// Checks if Deadlock game is the active foreground window, explicitly excluding our overlay.
fn is_deadlock_game_foreground(overlay_hwnd: HWND) -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() || (!overlay_hwnd.0.is_null() && hwnd == overlay_hwnd) {
            return false;
        }
        let mut title = [0u16; 128];
        let len = GetWindowTextW(hwnd, &mut title);
        if len > 0 {
            let title_str = String::from_utf16_lossy(&title[..len as usize]);
            if title_str.contains(WINDOW_TITLE) {
                return false;
            }
            if title_str == "Deadlock"
                || title_str.starts_with("Deadlock ")
                || title_str.contains("project8")
            {
                return true;
            }
        }
        false
    }
}

unsafe extern "system" fn enum_check_deadlock(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let mut title = [0u16; 128];
    let len = GetWindowTextW(hwnd, &mut title);
    if len > 0 {
        let title_str = String::from_utf16_lossy(&title[..len as usize]);
        if title_str != WINDOW_TITLE
            && (title_str == "Deadlock"
                || title_str.starts_with("Deadlock ")
                || title_str.contains("project8"))
        {
            let found = lparam.0 as *mut bool;
            *found = true;
            return BOOL(0);
        }
    }
    BOOL(1)
}

/// 100% passive Win32 query checking if the Deadlock game window currently exists.
fn is_deadlock_game_running() -> bool {
    unsafe {
        let mut found = false;
        let _ = EnumWindows(
            Some(enum_check_deadlock),
            LPARAM(&mut found as *mut bool as isize),
        );
        found
    }
}

fn main() -> eframe::Result<()> {
    // 0. Acquire passive Single-Instance lock
    let _instance_guard = match deadlock_overlay::tray::acquire_single_instance_lock(
        "Local\\DeadlockMacroOverlay_SingleInstance",
    ) {
        Some(guard) => guard,
        None => return Ok(()),
    };

    // 1. Detect screen resolution and primary DPI for default layout and scaling
    let (screen_w, screen_h) = unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        (if w > 0 { w } else { 1920 }, if h > 0 { h } else { 1080 })
    };

    let dpi_scale = unsafe {
        let hdc = GetDC(HWND(std::ptr::null_mut()));
        let dpi = if !hdc.0.is_null() {
            let d = GetDeviceCaps(hdc, LOGPIXELSX);
            let _ = ReleaseDC(HWND(std::ptr::null_mut()), hdc);
            d
        } else {
            96
        };
        if dpi > 0 { dpi as f32 / 96.0 } else { 1.0 }
    };

    let config_path = OverlayConfig::resolve_config_path();
    let config = OverlayConfig::load_or_create_with_dpi(&config_path, screen_w, screen_h, dpi_scale);
    let interactive_flag = Arc::new(AtomicBool::new(false)); // Start in ghost (click-through) mode
    let manual_override_flag = Arc::new(AtomicBool::new(false));
    let game_running_flag = Arc::new(AtomicBool::new(false));
    let roi_config = Arc::new(std::sync::RwLock::new(config.clone()));

    // 2. Setup channels
    let (ocr_tx, ocr_rx) = channel::<Option<u32>>();
    let (app_cmd_tx, app_cmd_rx) = channel::<AppCommand>();

    // 2b. Setup native System Tray bridge
    let (tray_tx, tray_rx) = channel::<deadlock_overlay::tray::TrayAction>();
    let cmd_tx_tray = app_cmd_tx.clone();
    thread::Builder::new()
        .name("tray_bridge".into())
        .spawn(move || {
            while let Ok(action) = tray_rx.recv() {
                match action {
                    deadlock_overlay::tray::TrayAction::ResetMatch => {
                        let _ = cmd_tx_tray.send(AppCommand::ResetMatch);
                    }
                    deadlock_overlay::tray::TrayAction::ReloadConfig => {
                        let _ = cmd_tx_tray.send(AppCommand::ReloadConfig);
                    }
                    deadlock_overlay::tray::TrayAction::ExitApp => {
                        let _ = cmd_tx_tray.send(AppCommand::ExitApp);
                    }
                }
            }
        })
        .expect("Failed to spawn tray bridge thread");
    let _tray_thread = deadlock_overlay::tray::spawn_system_tray(tray_tx);

    // 3. Background Input & In-Game Tab Passthrough thread (Window-scoped passive polling)
    let flag_clone = Arc::clone(&interactive_flag);
    let manual_clone = Arc::clone(&manual_override_flag);
    let cmd_tx_clone = app_cmd_tx.clone();
    let config_clone = Arc::clone(&roi_config);
    thread::Builder::new()
        .name("input_and_focus_manager".into())
        .spawn(move || {
            let mut cached_hwnd = HWND(std::ptr::null_mut());
            let mut prev_box_down = false;
            let mut prev_interactive_down = false;

            loop {
                // Poll for HWND if not yet found
                if cached_hwnd.0.is_null() {
                    unsafe {
                        cached_hwnd = FindWindowW(None, w!("DeadlockMacroOverlay"))
                            .unwrap_or(HWND(std::ptr::null_mut()));
                        if !cached_hwnd.0.is_null() {
                            strip_snap_styles(cached_hwnd);
                            let (init_x, init_y) = {
                                let cfg = config_clone.read().unwrap();
                                (cfg.win_x, cfg.win_y)
                            };
                            let _ = SetWindowPos(
                                cached_hwnd,
                                HWND_TOPMOST,
                                init_x,
                                init_y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                            );
                            apply_click_through(cached_hwnd, true);
                        }
                    }
                }

                // In-Game Tab Passthrough:
                // When in Deadlock and holding Tab, the game unlocks the mouse.
                // We automatically lift WS_EX_TRANSPARENT so the overlay becomes clickable!
                let in_deadlock = is_deadlock_game_foreground(cached_hwnd);
                let is_tab_held = in_deadlock && unsafe {
                    (GetAsyncKeyState(VK_TAB.0 as i32) as u16 & 0x8000) != 0
                };
                let manual_force = manual_clone.load(Ordering::Relaxed);
                let is_interactive = manual_force || is_tab_held || !in_deadlock;

                flag_clone.store(is_interactive, Ordering::Relaxed);
                if !cached_hwnd.0.is_null() {
                    apply_click_through(cached_hwnd, !is_interactive);
                }

                // Window-scoped passive hotkey polling:
                // Process game hotkeys when Deadlock is foreground, and allow calibration key (F10)
                // when either Deadlock or the overlay itself is foreground.
                // Prevents key hijacking in Discord, browser, or chat.
                let is_overlay_foreground = unsafe {
                    !cached_hwnd.0.is_null() && GetForegroundWindow() == cached_hwnd
                };

                if in_deadlock || is_overlay_foreground {
                    let (key_box, key_inter) = {
                        let cfg = config_clone.read().unwrap();
                        (cfg.key_box_timer, cfg.key_interactive)
                    };

                    let is_box_down = in_deadlock && unsafe { (GetAsyncKeyState(key_box as i32) as u16 & 0x8000) != 0 };
                    let is_inter_down = unsafe { (GetAsyncKeyState(key_inter as i32) as u16 & 0x8000) != 0 };

                    if is_box_down && !prev_box_down {
                        let _ = cmd_tx_clone.send(AppCommand::ToggleBoxTimer);
                    }
                    if is_inter_down && !prev_interactive_down {
                        let _ = cmd_tx_clone.send(AppCommand::ResetMatch);
                        let _ = cmd_tx_clone.send(AppCommand::ReloadConfig);
                    }

                    prev_box_down = is_box_down;
                    prev_interactive_down = is_inter_down;
                } else {
                    // Reset key states when neither Deadlock nor Overlay has focus
                    prev_box_down = false;
                    prev_interactive_down = false;
                }

                thread::sleep(Duration::from_millis(30));
            }
        })
        .expect("Failed to spawn focus manager thread");

    // 5. Background Screen Reader OCR thread (1 Hz) - Only active when Deadlock is running
    let roi_clone = Arc::clone(&roi_config);
    let game_running_ocr_clone = Arc::clone(&game_running_flag);
    thread::Builder::new()
        .name("ocr_reader".into())
        .spawn(move || {
            let ocr = ScreenOcr::new();

            loop {
                let is_running = is_deadlock_game_running();
                game_running_ocr_clone.store(is_running, Ordering::Relaxed);

                if !is_running {
                    let _ = ocr_tx.send(None);
                    thread::sleep(Duration::from_millis(1000));
                    continue;
                }

                let (rx, ry, rw, rh) = {
                    let cfg = roi_clone.read().unwrap();
                    (cfg.roi_x, cfg.roi_y, cfg.roi_w, cfg.roi_h)
                };

                let reading = if let Some(pixels) = capture_screen_roi(rx, ry, rw, rh) {
                    ocr.read_time_from_bgra(&pixels, rw, rh)
                } else {
                    None
                };

                let _ = ocr_tx.send(reading);
                thread::sleep(Duration::from_millis(400));
            }
        })
        .expect("Failed to spawn OCR thread");

    // 6. Native eframe window configuration (convert physical win_x/win_y to logical points using primary DPI)
    let (init_lx, init_ly) = config.logical_position(dpi_scale);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_maximize_button(false)
            .with_minimize_button(false)
            .with_inner_size([260.0, 185.0])
            .with_position([init_lx, init_ly]),
        ..Default::default()
    };

    eframe::run_native(
        WINDOW_TITLE,
        native_options,
        Box::new(move |_cc| {
            let mut app = DeadlockApp::new(
                config,
                config_path.to_string_lossy().into_owned(),
                interactive_flag,
                game_running_flag,
                ocr_rx,
            );
            app.screen_w = screen_w;
            app.screen_h = screen_h;

            // Wrap app to process commands from hotkey listener
            Ok(Box::new(AppWrapper {
                inner: app,
                cmd_rx: app_cmd_rx,
                roi_sync: roi_config,
                screen_w,
                screen_h,
            }))
        }),
    )
}

#[derive(Debug, Clone, Copy)]
enum AppCommand {
    ToggleBoxTimer,
    ResetMatch,
    ReloadConfig,
    ExitApp,
}

struct AppWrapper {
    inner: DeadlockApp,
    cmd_rx: std::sync::mpsc::Receiver<AppCommand>,
    roi_sync: Arc<std::sync::RwLock<OverlayConfig>>,
    screen_w: i32,
    screen_h: i32,
}

impl eframe::App for AppWrapper {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                AppCommand::ToggleBoxTimer => {
                    let now = self.inner.clock.current_seconds();
                    self.inner.scheduler.boxes.start_or_restart(now);
                    self.inner.audio.play_box_start();
                }
                AppCommand::ResetMatch => self.inner.reset_match(),
                AppCommand::ReloadConfig => self.inner.reload_config(self.screen_w, self.screen_h),
                AppCommand::ExitApp => {
                    deadlock_overlay::tray::remove_tray_icon();
                    ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                }
            }
        }

        // Sync any updated ROI from UI to background capture thread
        {
            if let Ok(mut lock) = self.roi_sync.write() {
                if *lock != self.inner.config {
                    *lock = self.inner.config.clone();
                }
            }
        }

        self.inner.update(ctx, frame);
    }
}
