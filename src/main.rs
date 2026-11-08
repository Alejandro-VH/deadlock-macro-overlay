#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use deadlock_overlay::app::DeadlockApp;
use deadlock_overlay::config::OverlayConfig;
use deadlock_overlay::screen_reader::{capture_screen_roi, ScreenOcr};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetForegroundWindow, GetSystemMetrics, GetWindowLongPtrW, GetWindowTextW,
    SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SM_CXSCREEN, SM_CYSCREEN, SWP_FRAMECHANGED,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_LAYERED, WS_EX_TRANSPARENT,
};

const WINDOW_TITLE: &str = "DeadlockMacroOverlay";
const CONFIG_FILE: &str = "deadlock_overlay.cfg";

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
            // Exclude our overlay window title
            if title_str.contains(WINDOW_TITLE) {
                return false;
            }
            // Match the actual Deadlock game window
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

fn main() -> eframe::Result<()> {
    // 1. Detect screen resolution for default layout
    let (screen_w, screen_h) = unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        (if w > 0 { w } else { 1920 }, if h > 0 { h } else { 1080 })
    };

    let config = OverlayConfig::load_from_file(CONFIG_FILE, screen_w, screen_h);
    let interactive_flag = Arc::new(AtomicBool::new(false)); // Start in ghost (click-through) mode
    let manual_override_flag = Arc::new(AtomicBool::new(false));
    let roi_config = Arc::new(std::sync::RwLock::new(config.clone()));

    // 2. Setup channels
    let (ocr_tx, ocr_rx) = channel::<Option<u32>>();
    let (app_cmd_tx, app_cmd_rx) = channel::<AppCommand>();

    // 3. Register Global Hotkeys
    let hotkey_manager = GlobalHotKeyManager::new().ok();
    let hk_f10 = HotKey::new(None, Code::F10);
    let hk_shift_f10 = HotKey::new(Some(Modifiers::SHIFT), Code::F10);
    let hk_f9 = HotKey::new(None, Code::F9);
    let hk_shift_f9 = HotKey::new(Some(Modifiers::SHIFT), Code::F9);

    if let Some(ref manager) = hotkey_manager {
        let _ = manager.register(hk_f10);
        let _ = manager.register(hk_shift_f10);
        let _ = manager.register(hk_f9);
        let _ = manager.register(hk_shift_f9);
    }

    // 4. Background Hotkey listener & Automatic Focus / Click-Through manager thread
    let flag_clone = Arc::clone(&interactive_flag);
    let manual_clone = Arc::clone(&manual_override_flag);
    let cmd_tx_clone = app_cmd_tx.clone();
    thread::Builder::new()
        .name("hotkey_and_focus_manager".into())
        .spawn(move || {
            let hotkey_channel = GlobalHotKeyEvent::receiver();
            let mut cached_hwnd = HWND(std::ptr::null_mut());

            loop {
                // Poll for HWND if not yet found
                if cached_hwnd.0.is_null() {
                    unsafe {
                        cached_hwnd = FindWindowW(None, w!("DeadlockMacroOverlay"))
                            .unwrap_or(HWND(std::ptr::null_mut()));
                        if !cached_hwnd.0.is_null() {
                            apply_click_through(cached_hwnd, true);
                        }
                    }
                }

                // Process hotkeys
                if let Ok(event) = hotkey_channel.try_recv() {
                    if event.id == hk_f10.id() {
                        let current_override = manual_clone.load(Ordering::Relaxed);
                        manual_clone.store(!current_override, Ordering::Relaxed);
                    } else if event.id == hk_shift_f10.id() {
                        let _ = cmd_tx_clone.send(AppCommand::ResetMatch);
                    } else if event.id == hk_f9.id() {
                        let _ = cmd_tx_clone.send(AppCommand::MidBossKill);
                    } else if event.id == hk_shift_f9.id() {
                        let _ = cmd_tx_clone.send(AppCommand::CancelMidBoss);
                    }
                }

                // Stable focus check:
                // Only Deadlock game forces ghost mode. Clicking on our overlay never flips it off!
                let in_deadlock = is_deadlock_game_foreground(cached_hwnd);
                let manual_force = manual_clone.load(Ordering::Relaxed);
                let is_interactive = manual_force || !in_deadlock;

                flag_clone.store(is_interactive, Ordering::Relaxed);
                if !cached_hwnd.0.is_null() {
                    apply_click_through(cached_hwnd, !is_interactive);
                }

                thread::sleep(Duration::from_millis(60));
            }
        })
        .expect("Failed to spawn focus manager thread");

    // 5. Background Screen Reader OCR thread (1 Hz)
    let roi_clone = Arc::clone(&roi_config);
    thread::Builder::new()
        .name("ocr_reader".into())
        .spawn(move || {
            let ocr = ScreenOcr::new();

            loop {
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
                thread::sleep(Duration::from_secs(1));
            }
        })
        .expect("Failed to spawn OCR thread");

    // 6. Native eframe window configuration
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_inner_size([250.0, 68.0])
            .with_position([config.win_x as f32, config.win_y as f32]),
        ..Default::default()
    };

    eframe::run_native(
        WINDOW_TITLE,
        native_options,
        Box::new(move |_cc| {
            let app = DeadlockApp::new(
                config,
                CONFIG_FILE.to_string(),
                interactive_flag,
                ocr_rx,
            );

            // Wrap app to process commands from hotkey listener
            Ok(Box::new(AppWrapper {
                inner: app,
                cmd_rx: app_cmd_rx,
                roi_sync: roi_config,
            }))
        }),
    )
}

#[derive(Debug, Clone, Copy)]
enum AppCommand {
    ResetMatch,
    MidBossKill,
    CancelMidBoss,
}

struct AppWrapper {
    inner: DeadlockApp,
    cmd_rx: std::sync::mpsc::Receiver<AppCommand>,
    roi_sync: Arc<std::sync::RwLock<OverlayConfig>>,
}

impl eframe::App for AppWrapper {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                AppCommand::ResetMatch => self.inner.reset_match(),
                AppCommand::MidBossKill => {
                    let now = self.inner.clock.current_seconds();
                    self.inner.scheduler.midboss.record_kill(now);
                }
                AppCommand::CancelMidBoss => {
                    self.inner.scheduler.midboss.cancel_kill();
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
