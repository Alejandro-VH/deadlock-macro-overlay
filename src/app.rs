use crate::audio::AudioNotifier;
use crate::config::{key_to_display_name, OverlayConfig};
use crate::hero_tracker::HeroTracker;
use crate::timers::{format_time_mmss, ClockFilter, EventKind, EventStatus, MacroEvent, MacroScheduler};
use eframe::egui::{self, Color32, Margin, RichText, Rounding, Stroke, Vec2};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, ReleaseCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, GetWindowRect, SendMessageW, HTCAPTION, WM_NCLBUTTONDOWN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebindTarget {
    BoxTimer,
    MidBoss,
    Interactive,
}

pub fn poll_key_for_rebinding() -> Option<u16> {
    unsafe {
        // Esc cancels rebinding
        if (GetAsyncKeyState(0x1B) as u16 & 0x8000) != 0 {
            return Some(0);
        }
        // Mouse side buttons
        if (GetAsyncKeyState(0x05) as u16 & 0x8000) != 0 {
            return Some(0x05);
        }
        if (GetAsyncKeyState(0x06) as u16 & 0x8000) != 0 {
            return Some(0x06);
        }
        if (GetAsyncKeyState(0x04) as u16 & 0x8000) != 0 {
            return Some(0x04);
        }
        // Function keys F1..F12
        for vk in 0x70..=0x7B {
            if (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 {
                return Some(vk as u16);
            }
        }
        // Letters A..Z
        for vk in 0x41..=0x5A {
            if (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 {
                return Some(vk as u16);
            }
        }
        // Digits 0..9
        for vk in 0x30..=0x39 {
            if (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 {
                return Some(vk as u16);
            }
        }
        // Modifiers / special keys: CapsLock (0x14), Space (0x20), Alt (0x12), Ctrl (0x11), Shift (0x10)
        // Note: Tab (0x09) is excluded to prevent accidental assignment during in-game Tab passthrough
        for &vk in &[0x14, 0x20, 0x12, 0x11, 0x10] {
            if (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 {
                return Some(vk as u16);
            }
        }
    }
    None
}

/// Triggers smooth, zero-latency native Windows window dragging via DWM using actual cursor coordinates.
pub fn start_native_window_drag() -> Option<(i32, i32)> {
    unsafe {
        let hwnd = FindWindowW(None, w!("DeadlockMacroOverlay")).ok()?;
        if hwnd.0.is_null() {
            return None;
        }

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let lparam = (((pt.y as u16 as usize) << 16) | (pt.x as u16 as usize)) as isize;

        let _ = ReleaseCapture();
        SendMessageW(
            hwnd,
            WM_NCLBUTTONDOWN,
            WPARAM(HTCAPTION as usize),
            LPARAM(lparam),
        );

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            Some((rect.left, rect.top))
        } else {
            None
        }
    }
}

pub struct DeadlockApp {
    pub config: OverlayConfig,
    pub config_path: String,
    pub clock: ClockFilter,
    pub scheduler: MacroScheduler,
    pub audio: AudioNotifier,
    pub interactive_mode: Arc<AtomicBool>,
    pub game_running: Arc<AtomicBool>,
    pub ocr_rx: Receiver<Option<u32>>,
    pub triggered_alerts: HashSet<(EventKind, u32)>, // (kind, target_time)
    pub triggered_warnings: HashSet<(EventKind, u32)>,
    pub last_tick_instant: Instant,
    pub hero_tracker: HeroTracker,
    pub config_menu_open: bool,
    pub rebinding: Option<RebindTarget>,
    pub last_box_rem: Option<u32>,
    pub last_boss_rem: Option<u32>,
    pub last_log_poll_instant: Instant,
    pub current_window_size: Option<Vec2>,
}

impl DeadlockApp {
    pub fn new(
        config: OverlayConfig,
        config_path: String,
        interactive_mode: Arc<AtomicBool>,
        game_running: Arc<AtomicBool>,
        ocr_rx: Receiver<Option<u32>>,
    ) -> Self {
        let mut hero_tracker = HeroTracker::new(None);
        if !config.last_hero.is_empty() && config.last_hero != "Auto" {
            hero_tracker.set_manual_hero(&config.last_hero);
        }
        let audio = AudioNotifier::with_muted(config.is_muted);

        Self {
            config,
            config_path,
            clock: ClockFilter::new(),
            scheduler: MacroScheduler::default(),
            audio,
            interactive_mode,
            game_running,
            ocr_rx,
            triggered_alerts: HashSet::new(),
            triggered_warnings: HashSet::new(),
            last_tick_instant: Instant::now(),
            hero_tracker,
            config_menu_open: false,
            rebinding: None,
            last_box_rem: None,
            last_boss_rem: None,
            last_log_poll_instant: Instant::now(),
            current_window_size: None,
        }
    }

    /// Pulls latest OCR readings from background worker and advances local clock.
    pub fn update_timing(&mut self) {
        let mut latest_reading = None;
        while let Ok(msg) = self.ocr_rx.try_recv() {
            latest_reading = Some(msg);
        }
        if let Some(reading) = latest_reading {
            if self.clock.feed_ocr_reading(reading) {
                self.last_tick_instant = Instant::now();
            }
        }

        if self.last_tick_instant.elapsed() >= Duration::from_secs(1) {
            self.clock.tick_second();
            self.last_tick_instant = Instant::now();
        }

        // Poll Deadlock console.log every 1 second for hero spawns and match disconnect
        if self.last_log_poll_instant.elapsed() >= Duration::from_secs(1) {
            let (_, match_ended) = self.hero_tracker.poll_log_events();
            if match_ended {
                self.reset_match();
            }
            self.last_log_poll_instant = Instant::now();
        }
    }

    /// Evaluates scheduled events and fires synthetic harmonic audio alerts when crossing thresholds.
    pub fn process_alerts(&mut self, events: &[MacroEvent]) {
        let now = self.clock.current_seconds();
        let mut trigger_warn = false;
        let mut trigger_active = false;

        for event in events {
            let key = (event.kind, event.target_time);
            let status = event.status(now);

            if status == EventStatus::Warning && !self.triggered_warnings.contains(&key) {
                self.triggered_warnings.insert(key);
                if self.config.alert_30s_sound {
                    trigger_warn = true;
                }
            } else if status == EventStatus::Active && !self.triggered_alerts.contains(&key) {
                self.triggered_alerts.insert(key);
                trigger_active = true;
            }
        }

        if trigger_active {
            self.audio.play_active();
        } else if trigger_warn {
            self.audio.play_warning();
        }
    }

    pub fn reset_match(&mut self) {
        self.clock.reset();
        self.scheduler.midboss.reset();
        self.scheduler.boxes.reset();
        self.triggered_alerts.clear();
        self.triggered_warnings.clear();
        self.hero_tracker.reset_hero_state();
        self.last_box_rem = None;
        self.last_boss_rem = None;
    }
}

impl eframe::App for DeadlockApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Purge Tab key events from egui to completely eliminate focus cycling / outline box jumping
        ctx.input_mut(|i| {
            i.events.retain(|e| !matches!(e, egui::Event::Key { key: egui::Key::Tab, .. }));
        });

        // 2. Disable native keyboard zoom (Ctrl +, Ctrl -, Ctrl 0)
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
        });

        // 3. Disable text selection and widget focus expansion rings
        ctx.style_mut(|s| {
            s.interaction.selectable_labels = false;
            s.visuals.widgets.active.expansion = 0.0;
            s.visuals.widgets.hovered.expansion = 0.0;
        });

        // 4. Apply configured HUD scale via zoom factor to preserve native OS DPI scaling
        ctx.set_zoom_factor(self.config.hud_scale);

        let is_game_running = self.game_running.load(Ordering::Relaxed);
        let is_interactive = self.interactive_mode.load(Ordering::Relaxed);

        // Automatic drawer and rebinding state management (only force close during active gameplay)
        if is_game_running && !is_interactive {
            self.config_menu_open = false;
            self.rebinding = None;
        }

        // Process key rebinding if active
        if let Some(target) = self.rebinding {
            if let Some(vk) = poll_key_for_rebinding() {
                if vk == 0 {
                    // Esc pressed: cancel rebinding
                    self.rebinding = None;
                } else {
                    match target {
                        RebindTarget::BoxTimer => self.config.key_box_timer = vk,
                        RebindTarget::MidBoss => self.config.key_midboss = vk,
                        RebindTarget::Interactive => self.config.key_interactive = vk,
                    }
                    let _ = self.config.save_to_file(&self.config_path);
                    self.rebinding = None;
                }
            }
        }

        if is_game_running {
            self.update_timing();
        } else {
            // Game not running: drain OCR messages and keep clock reset
            while self.ocr_rx.try_recv().is_ok() {}
            self.clock.reset();
            self.triggered_alerts.clear();
            self.triggered_warnings.clear();
        }

        let current_time = self.clock.current_seconds();

        // Check completion transitions for box and boss audio chimes
        if is_game_running {
            let box_rem = self.scheduler.boxes.get_remaining(current_time);
            if let Some(rem) = box_rem {
                if rem == 0 && self.last_box_rem.is_some_and(|r| r > 0) {
                    self.audio.play_box_ready();
                }
            }
            self.last_box_rem = box_rem;

            let boss_rem = self
                .scheduler
                .midboss
                .get_active_respawn(current_time)
                .map(|r| r.saturating_sub(current_time));
            if let Some(rem) = boss_rem {
                if rem == 0 && self.last_boss_rem.is_some_and(|r| r > 0) {
                    self.audio.play_boss_ready();
                }
            }
            self.last_boss_rem = boss_rem;
        }

        let (immediate_group, next_group) = if is_game_running {
            self.scheduler.get_grouped_horizons(current_time)
        } else {
            (None, None)
        };

        if let Some(ref grp) = immediate_group {
            self.process_alerts(&grp.events);
        }

        // Dynamic geometry: Tactical Noir Pro layout sizing
        let target_size = if self.config_menu_open && (is_interactive || !is_game_running) {
            Vec2::new(245.0, 225.0)
        } else if !is_game_running {
            Vec2::new(245.0, 65.0)
        } else {
            let event_count = immediate_group.as_ref().map_or(1, |g| g.events.len().max(1));
            let has_next = next_group.is_some();
            let h = 108.0 + 18.0 + (event_count as f32 * 18.0) + (if has_next { 34.0 } else { 0.0 });
            Vec2::new(245.0, h.clamp(150.0, 235.0))
        };

        if self.current_window_size != Some(target_size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
            self.current_window_size = Some(target_size);
        }

        // Tactical Noir Pro styling: deep charcoal glassmorphism with configurable opacity
        let alpha = (self.config.bg_opacity * 255.0).round().clamp(50.0, 255.0) as u8;
        let frame = egui::Frame::none()
            .fill(Color32::from_rgba_premultiplied(13, 17, 23, alpha))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(37, 45, 61)))
            .inner_margin(Margin::symmetric(8.0, 6.0))
            .rounding(Rounding::same(6.0));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            // 1. TOP MICRO PROGRESS BAR (2px) - Only visible when game is actively running
            if is_game_running {
                let (bar_rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 2.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    bar_rect,
                    Rounding::same(1.0),
                    Color32::from_rgb(22, 27, 34),
                );

                if let Some(ref group) = immediate_group {
                    if let Some(first_ev) = group.events.first() {
                        let progress = first_ev.progress(current_time);
                        let bar_color = match group.status {
                            EventStatus::Active => Color32::from_rgb(72, 239, 128),
                            EventStatus::Warning => Color32::from_rgb(251, 146, 60),
                            _ => Color32::from_rgb(245, 158, 11),
                        };
                        let filled_w = (bar_rect.width() * progress).clamp(0.0, bar_rect.width());
                        let filled_rect = egui::Rect::from_min_size(
                            bar_rect.min,
                            Vec2::new(filled_w, bar_rect.height()),
                        );
                        ui.painter()
                            .rect_filled(filled_rect, Rounding::same(1.0), bar_color);
                    }
                }
                ui.add_space(3.0);
            }

            // 2. HEADER ROW
            ui.horizontal(|ui| {
                let clock_text = if !is_game_running {
                    "--:--".to_string()
                } else if self.clock.is_active() {
                    format_time_mmss(current_time)
                } else {
                    "--:--".to_string()
                };

                let clock_color = if !is_game_running {
                    Color32::from_rgb(100, 116, 139)
                } else {
                    Color32::from_rgb(245, 158, 11)
                };

                ui.label(
                    RichText::new(clock_text)
                        .strong()
                        .color(clock_color)
                        .size(13.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close button [✕] - Only visible when Deadlock is NOT running
                    if !is_game_running {
                        let close_btn = ui.add(
                            egui::Button::new(
                                RichText::new("✕")
                                    .size(11.0)
                                    .color(Color32::from_rgb(239, 68, 68)),
                            )
                            .frame(false),
                        );
                        if close_btn.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if close_btn.hovered() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        }
                    }

                    // Native painter-drawn grip dots (6-dot matrix: 2 columns of 3 dots, 1.2px radius)
                    let (handle_rect, handle_response) = ui.allocate_exact_size(
                        Vec2::new(14.0, 14.0),
                        egui::Sense::click_and_drag(),
                    );
                    let is_hovered = handle_response.hovered();
                    let dot_color = if is_hovered {
                        Color32::from_rgb(226, 232, 240)
                    } else {
                        Color32::from_rgb(100, 116, 139)
                    };
                    let cx = handle_rect.center().x;
                    let cy = handle_rect.center().y;
                    for dx in &[-2.5_f32, 2.5_f32] {
                        for dy in &[-4.0_f32, 0.0_f32, 4.0_f32] {
                            ui.painter().circle_filled(
                                egui::pos2(cx + dx, cy + dy),
                                1.2,
                                dot_color,
                            );
                        }
                    }

                    if handle_response.is_pointer_button_down_on()
                        || handle_response.drag_started()
                    {
                        if let Some((new_x, new_y)) = start_native_window_drag() {
                            self.config.win_x = new_x;
                            self.config.win_y = new_y;
                            let _ = self.config.save_to_file(&self.config_path);
                        }
                    }

                    // Config Drawer Toggle [⚙] (Interactive mode or Standby)
                    if is_interactive || !is_game_running {
                        let cfg_color = if self.config_menu_open {
                            Color32::from_rgb(245, 158, 11)
                        } else {
                            Color32::from_rgb(148, 163, 184)
                        };
                        let cfg_btn = ui.add(
                            egui::Button::new(
                                RichText::new("⚙").size(12.0).color(cfg_color),
                            )
                            .frame(false),
                        );
                        if cfg_btn.clicked() {
                            self.config_menu_open = !self.config_menu_open;
                            self.rebinding = None;
                        }
                        if cfg_btn.hovered() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        }
                    }

                    // In-Game Only Controls: Mute & Reset
                    if is_game_running {
                        // Mute Toggle (Text: Vol / Mute)
                        let is_muted = self.audio.is_muted();
                        let mute_label = if is_muted { "Mute" } else { "Vol" };
                        let mute_color = if is_muted {
                            Color32::from_rgb(239, 68, 68)
                        } else {
                            Color32::from_rgb(148, 163, 184)
                        };

                        if is_interactive {
                            let mute_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(mute_label).size(10.5).color(mute_color),
                                )
                                .frame(false),
                            );
                            if mute_btn.clicked() {
                                let new_muted = self.audio.toggle_mute();
                                self.config.is_muted = new_muted;
                                let _ = self.config.save_to_file(&self.config_path);
                            }
                            if mute_btn.hovered() {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                            }
                        } else if is_muted {
                            ui.label(
                                RichText::new("Mute")
                                    .size(10.0)
                                    .color(Color32::from_rgb(239, 68, 68)),
                            );
                        }

                        // Reset Match Button [↺]
                        if is_interactive {
                            let reset_btn = ui.add(
                                egui::Button::new(
                                    RichText::new("↺")
                                        .size(12.0)
                                        .color(Color32::from_rgb(148, 163, 184)),
                                )
                                .frame(false),
                            );
                            if reset_btn.clicked() {
                                self.reset_match();
                            }
                            if reset_btn.hovered() {
                                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                            }
                        }
                    }
                });
            });

            // 3. MAIN BODY: Config Drawer OR Tactical Noir Pro Layout
            if self.config_menu_open && (is_interactive || !is_game_running) {
                ui.add_space(3.0);
                ui.separator();
                ui.label(
                    RichText::new("Configuración")
                        .strong()
                        .size(11.0)
                        .color(Color32::from_rgb(245, 158, 11)),
                );
                ui.add_space(2.0);

                // Rebind Box Timer
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Ruta Cajas:")
                            .size(10.0)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                    let btn_text = if self.rebinding == Some(RebindTarget::BoxTimer) {
                        "Presiona tecla...".to_string()
                    } else {
                        key_to_display_name(self.config.key_box_timer)
                    };
                    let btn = ui.add(egui::Button::new(RichText::new(btn_text).size(9.5)));
                    if btn.clicked() {
                        self.rebinding = Some(RebindTarget::BoxTimer);
                    }
                });

                // Rebind Mid Boss
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Mid Boss:")
                            .size(10.0)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                    let btn_text = if self.rebinding == Some(RebindTarget::MidBoss) {
                        "Presiona tecla...".to_string()
                    } else {
                        key_to_display_name(self.config.key_midboss)
                    };
                    let btn = ui.add(egui::Button::new(RichText::new(btn_text).size(9.5)));
                    if btn.clicked() {
                        self.rebinding = Some(RebindTarget::MidBoss);
                    }
                });

                // Rebind Interactive Mode Override
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Modo Calibrar:")
                            .size(10.0)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                    let btn_text = if self.rebinding == Some(RebindTarget::Interactive) {
                        "Presiona tecla...".to_string()
                    } else {
                        key_to_display_name(self.config.key_interactive)
                    };
                    let btn = ui.add(egui::Button::new(RichText::new(btn_text).size(9.5)));
                    if btn.clicked() {
                        self.rebinding = Some(RebindTarget::Interactive);
                    }
                });

                ui.add_space(2.0);

                // 30s Audio Alert Toggle
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Aviso 30s:")
                            .size(10.0)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                    if ui.selectable_label(self.config.alert_30s_sound, "On").clicked() {
                        self.config.alert_30s_sound = true;
                        let _ = self.config.save_to_file(&self.config_path);
                    }
                    if ui.selectable_label(!self.config.alert_30s_sound, "Off").clicked() {
                        self.config.alert_30s_sound = false;
                        let _ = self.config.save_to_file(&self.config_path);
                    }
                });

                // HUD Scale Selector
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Escala:")
                            .size(10.0)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                    for (scale_val, label) in &[(1.0_f32, "100%"), (1.15_f32, "115%"), (1.30_f32, "130%")] {
                        let selected = (self.config.hud_scale - scale_val).abs() < 0.01;
                        if ui.selectable_label(selected, *label).clicked() {
                            self.config.hud_scale = *scale_val;
                            self.current_window_size = None;
                            let _ = self.config.save_to_file(&self.config_path);
                        }
                    }
                });

                // Background Opacity Selector
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Opacidad:")
                            .size(10.0)
                            .color(Color32::from_rgb(203, 213, 225)),
                    );
                    for (op_val, label) in &[(0.92_f32, "Alta"), (0.75_f32, "Media"), (0.50_f32, "Baja")] {
                        let selected = (self.config.bg_opacity - op_val).abs() < 0.01;
                        if ui.selectable_label(selected, *label).clicked() {
                            self.config.bg_opacity = *op_val;
                            let _ = self.config.save_to_file(&self.config_path);
                        }
                    }
                });

                ui.add_space(2.0);
                ui.label(
                    RichText::new("(Esc para cancelar rebind)")
                        .size(8.5)
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            } else {
                ui.add_space(2.0);

                if !is_game_running {
                    // Centered Standby View
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("●")
                                .size(8.0)
                                .color(Color32::from_rgb(245, 158, 11)),
                        );
                        ui.add_space(3.0);
                        ui.label(
                            RichText::new("En espera de Deadlock...")
                                .size(11.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    });
                } else {
                    // Tactical Noir Pro: Immediate Milestone Sub-card
                    if let Some(ref group) = immediate_group {
                        let header_color = match group.status {
                            EventStatus::Active => Color32::from_rgb(72, 239, 128),
                            EventStatus::Warning => Color32::from_rgb(251, 146, 60),
                            _ => Color32::from_rgb(245, 158, 11),
                        };

                        let target_str = format_time_mmss(group.target_time);

                        let card_bg = Color32::from_rgba_premultiplied(22, 27, 34, alpha);
                        let card_stroke = Stroke::new(1.0_f32, Color32::from_rgb(37, 45, 61));
                        let card_frame = egui::Frame::none()
                            .fill(card_bg)
                            .stroke(card_stroke)
                            .rounding(Rounding::same(4.0))
                            .inner_margin(Margin::symmetric(7.0, 5.0));

                        card_frame.show(ui, |ui| {
                            // Sub-card Header: Accent bar + Timestamp + Pill Badge
                            ui.horizontal(|ui| {
                                let (bar_r, _) = ui.allocate_exact_size(Vec2::new(3.0, 14.0), egui::Sense::hover());
                                ui.painter().rect_filled(
                                    bar_r,
                                    Rounding::same(1.5),
                                    header_color,
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(target_str)
                                        .strong()
                                        .size(12.0)
                                        .color(header_color),
                                );

                                // Right-aligned pill badge
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if group.status == EventStatus::Active {
                                        let badge_bg = Color32::from_rgba_premultiplied(16, 50, 30, 220);
                                        let badge_stroke = Stroke::new(1.0_f32, Color32::from_rgb(72, 239, 128));
                                        egui::Frame::none()
                                            .fill(badge_bg)
                                            .stroke(badge_stroke)
                                            .rounding(Rounding::same(3.0))
                                            .inner_margin(Margin::symmetric(4.0, 1.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new("● ACTIVO")
                                                        .size(8.5)
                                                        .strong()
                                                        .color(Color32::from_rgb(72, 239, 128)),
                                                );
                                            });
                                    } else if group.status == EventStatus::Warning {
                                        let badge_bg = Color32::from_rgba_premultiplied(60, 30, 10, 220);
                                        let badge_stroke = Stroke::new(1.0_f32, Color32::from_rgb(251, 146, 60));
                                        egui::Frame::none()
                                            .fill(badge_bg)
                                            .stroke(badge_stroke)
                                            .rounding(Rounding::same(3.0))
                                            .inner_margin(Margin::symmetric(4.0, 1.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new("PRONTO")
                                                        .size(8.5)
                                                        .strong()
                                                        .color(Color32::from_rgb(251, 146, 60)),
                                                );
                                            });
                                    }
                                });
                            });

                            // Bulleted list of simultaneous events inside the card
                            for event in &group.events {
                                ui.horizontal(|ui| {
                                    ui.add_space(8.0);
                                    ui.label(RichText::new("•").size(10.0).color(header_color));
                                    ui.label(
                                        RichText::new(event.kind.display_name())
                                            .size(10.0)
                                            .color(Color32::from_rgb(226, 232, 240)),
                                    );
                                });
                            }
                        });
                    } else {
                        ui.label(
                            RichText::new("Sin eventos pendientes")
                                .color(Color32::from_rgb(148, 163, 184))
                                .size(11.0),
                        );
                    }

                    // Subsequent Milestone Horizon Preview
                    if let Some(ref next) = next_group {
                        ui.add_space(2.0);
                        let nxt_target_str = format_time_mmss(next.target_time);
                        let names: Vec<&str> =
                            next.events.iter().map(|e| e.kind.display_name()).collect();
                        let names_str = names.join(" · ");

                        ui.horizontal(|ui| {
                            egui::Frame::none()
                                .fill(Color32::from_rgb(30, 37, 48))
                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 55, 72)))
                                .rounding(Rounding::same(3.0))
                                .inner_margin(Margin::symmetric(4.0, 1.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("SIG {}", nxt_target_str))
                                            .size(8.5)
                                            .strong()
                                            .color(Color32::from_rgb(148, 163, 184)),
                                    );
                                });
                            ui.add_space(3.0);
                            ui.label(
                                RichText::new(names_str)
                                    .size(9.5)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            );
                        });
                    }

                    // Divider before fixed bottom farm trackers
                    ui.add_space(2.0);
                    ui.separator();
                    ui.add_space(1.0);

                    // Fixed Bottom Tracker 1: Box Route Tracker
                    let box_key_name = key_to_display_name(self.config.key_box_timer);
                    let box_rem = self.scheduler.boxes.get_remaining(current_time);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Ruta Cajas")
                                .size(10.0)
                                .color(Color32::from_rgb(203, 213, 225)),
                        );
                        // Keycap [F8]
                        egui::Frame::none()
                            .fill(Color32::from_rgb(30, 37, 48))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 65, 81)))
                            .rounding(Rounding::same(3.0))
                            .inner_margin(Margin::symmetric(4.0, 1.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(box_key_name)
                                        .size(8.5)
                                        .monospace()
                                        .color(Color32::from_rgb(226, 232, 240)),
                                );
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(rem) = box_rem {
                                if rem == 0 {
                                    ui.label(
                                        RichText::new("LISTO")
                                            .strong()
                                            .size(10.0)
                                            .color(Color32::from_rgb(72, 239, 128)),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("{:02}:{:02}", rem / 60, rem % 60))
                                            .strong()
                                            .monospace()
                                            .size(10.0)
                                            .color(Color32::from_rgb(245, 158, 11)),
                                    );
                                }
                            } else {
                                ui.label(
                                    RichText::new("LISTO")
                                        .size(10.0)
                                        .color(Color32::from_rgb(100, 116, 139)),
                                );
                            }
                        });
                    });

                    // Fixed Bottom Tracker 2: Mid Boss Tracker
                    let boss_key_name = key_to_display_name(self.config.key_midboss);
                    let boss_respawn = self.scheduler.midboss.get_active_respawn(current_time);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Mid Boss")
                                .size(10.0)
                                .color(Color32::from_rgb(203, 213, 225)),
                        );
                        // Keycap [F9]
                        egui::Frame::none()
                            .fill(Color32::from_rgb(30, 37, 48))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 65, 81)))
                            .rounding(Rounding::same(3.0))
                            .inner_margin(Margin::symmetric(4.0, 1.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(boss_key_name)
                                        .size(8.5)
                                        .monospace()
                                        .color(Color32::from_rgb(226, 232, 240)),
                                );
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(respawn) = boss_respawn {
                                let rem = respawn.saturating_sub(current_time);
                                if rem == 0 {
                                    ui.label(
                                        RichText::new("LISTO")
                                            .strong()
                                            .size(10.0)
                                            .color(Color32::from_rgb(72, 239, 128)),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("{:02}:{:02}", rem / 60, rem % 60))
                                            .strong()
                                            .monospace()
                                            .size(10.0)
                                            .color(Color32::from_rgb(251, 146, 60)),
                                    );
                                }
                            } else {
                                ui.label(
                                    RichText::new("LISTO")
                                        .size(10.0)
                                        .color(Color32::from_rgb(100, 116, 139)),
                                );
                            }
                        });
                    });
                }
            }
        });

        // Fast repaint in calibration/config mode; low CPU (200ms) in ghost mode
        if is_interactive || self.config_menu_open {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}
