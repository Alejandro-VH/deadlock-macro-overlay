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
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebindTarget {
    BoxTimer,
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
    pub last_log_poll_instant: Instant,
    pub current_window_size: Option<Vec2>,
    pub last_applied_scale: f32,
    pub last_applied_ppp: f32,
    pub screen_w: i32,
    pub screen_h: i32,
    pub position_synced: bool,
    pub was_dragged: bool,
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
        let initial_scale = config.hud_scale;

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
            last_log_poll_instant: Instant::now(),
            current_window_size: None,
            last_applied_scale: initial_scale,
            last_applied_ppp: 1.0,
            screen_w: 1920,
            screen_h: 1080,
            position_synced: false,
            was_dragged: false,
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
    }

    pub fn reload_config(&mut self, screen_w: i32, screen_h: i32) {
        self.screen_w = screen_w;
        self.screen_h = screen_h;
        self.config = OverlayConfig::load_or_create(&self.config_path, screen_w, screen_h);
        if !self.config.last_hero.is_empty() && self.config.last_hero != "Auto" {
            self.hero_tracker.set_manual_hero(&self.config.last_hero);
        }
        self.audio.set_muted(self.config.is_muted);
        self.current_window_size = None;
        self.position_synced = false;
    }
}

impl eframe::App for DeadlockApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Purge Tab key events from egui to completely eliminate focus cycling / outline box jumping
        ctx.input_mut(|i| {
            i.events.retain(|e| !matches!(e, egui::Event::Key { key: egui::Key::Tab, .. }));
        });

        // 2. Disable native keyboard zoom (Ctrl +, Ctrl -, Ctrl 0)
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
        });

        // 3. Disable text selection and widget focus expansion rings; enforce transparent backdrop
        ctx.style_mut(|s| {
            s.interaction.selectable_labels = false;
            s.visuals.window_fill = Color32::TRANSPARENT;
            s.visuals.panel_fill = Color32::TRANSPARENT;
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

        // Check completion transitions for box route audio chime
        if is_game_running {
            let box_rem = self.scheduler.boxes.get_remaining(current_time);
            if let Some(rem) = box_rem {
                if rem == 0 && self.last_box_rem.is_some_and(|r| r > 0) {
                    self.audio.play_box_ready();
                }
            }
            self.last_box_rem = box_rem;
        }

        let (immediate_group, next_group) = if is_game_running {
            self.scheduler.get_grouped_horizons(current_time)
        } else {
            (None, None)
        };

        if let Some(ref grp) = immediate_group {
            self.process_alerts(&grp.events);
        }

        // Dynamic geometry: Clean HUD layout sizing
        let target_size = if self.config_menu_open && (is_interactive || !is_game_running) {
            Vec2::new(260.0, 240.0)
        } else if !is_game_running {
            Vec2::new(260.0, 68.0)
        } else {
            let event_count = immediate_group.as_ref().map_or(1, |g| g.events.len().max(1));
            let show_next = !self.config.minimal_hud && next_group.is_some();
            let next_count = if show_next {
                next_group.as_ref().map_or(0, |g| g.events.len())
            } else {
                0
            };
            let next_h = if show_next {
                24.0 + (next_count as f32 * 18.0)
            } else {
                0.0
            };
            // 46px base (header + micro progress) + immediate events + next events + 12px safety bottom margin
            let h = 46.0 + 18.0 + (event_count as f32 * 18.0) + next_h + 12.0;
            Vec2::new(260.0, h.clamp(72.0, 320.0))
        };

        let ppp = ctx.pixels_per_point();
        let scale_changed = (self.last_applied_scale - self.config.hud_scale).abs() > 0.005;
        let ppp_changed = (self.last_applied_ppp - ppp).abs() > 0.005;
        let actual_size = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
        let size_desynced = match actual_size {
            Some(actual) => (actual.x - target_size.x).abs() > 4.0 || (actual.y - target_size.y).abs() > 4.0,
            None => false,
        };

        let phys_w = (target_size.x * ppp).ceil() as i32;
        let phys_h = (target_size.y * ppp).ceil() as i32;

        if !self.position_synced || scale_changed || ppp_changed {
            let orig_x = self.config.win_x;
            let orig_y = self.config.win_y;
            self.config.clamp_to_screen_with_size(self.screen_w, self.screen_h, phys_w, phys_h);
            if self.config.win_x != orig_x || self.config.win_y != orig_y {
                let _ = self.config.save_to_file(&self.config_path);
            }
            let (lx, ly) = self.config.logical_position(ppp);
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(lx, ly)));
            self.position_synced = true;
        }

        if self.current_window_size != Some(target_size) || scale_changed || ppp_changed || size_desynced {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(target_size));
            self.current_window_size = Some(target_size);
            self.last_applied_scale = self.config.hud_scale;
            self.last_applied_ppp = ppp;
        }

        // Clean styling: charcoal glassmorphism with mathematically sound unmultiplied alpha
        let alpha = (self.config.bg_opacity * 255.0).round().clamp(50.0, 255.0) as u8;
        let frame = egui::Frame::none()
            .fill(Color32::from_rgba_unmultiplied(13, 17, 23, alpha))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(37, 45, 61, alpha)))
            .inner_margin(Margin::symmetric(8.0, 6.0))
            .rounding(Rounding::same(6.0));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.set_max_width(ui.available_width());
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

                let clock_resp = ui.add(
                    egui::Label::new(
                        RichText::new(clock_text)
                            .strong()
                            .color(clock_color)
                            .size(13.0),
                    )
                    .sense(egui::Sense::drag()),
                );
                if clock_resp.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                }
                if clock_resp.drag_started_by(egui::PointerButton::Primary) {
                    self.was_dragged = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close button [×] - Only visible when Deadlock is NOT running
                    if !is_game_running {
                        let (close_rect, close_resp) =
                            ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::click());
                        if close_resp.clicked() {
                            crate::tray::remove_tray_icon();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let x_color = if close_resp.hovered() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                            Color32::from_rgb(248, 113, 113)
                        } else {
                            Color32::from_rgb(239, 68, 68)
                        };
                        let center = close_rect.center();
                        let half = 3.5_f32;
                        let stroke = Stroke::new(1.5_f32, x_color);
                        ui.painter().line_segment(
                            [center - Vec2::new(half, half), center + Vec2::new(half, half)],
                            stroke,
                        );
                        ui.painter().line_segment(
                            [center - Vec2::new(half, -half), center + Vec2::new(half, -half)],
                            stroke,
                        );
                        ui.add_space(2.0);
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

                    // Natural Header Drag Zone (Remaining empty space in the header bar)
                    let avail = ui.available_size_before_wrap();
                    if avail.x > 0.0 && avail.y > 0.0 {
                        let drag_area = ui.allocate_response(avail, egui::Sense::drag());
                        if drag_area.hovered() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                        }
                        if drag_area.drag_started_by(egui::PointerButton::Primary) {
                            self.was_dragged = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
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

                // Rebind Reload Overlay Hotkey
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Recargar Overlay:")
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
                    RichText::new("Esc para cancelar")
                        .size(8.5)
                        .color(Color32::from_rgb(148, 163, 184)),
                );
            } else {
                ui.add_space(2.0);

                if !is_game_running {
                    // Centered Standby View with vector-painted status dot
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
                        ui.painter().circle_filled(
                            dot_rect.center(),
                            3.0,
                            Color32::from_rgb(245, 158, 11),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("En espera de Deadlock...")
                                .size(11.0)
                                .color(Color32::from_rgb(148, 163, 184)),
                        );
                    });
                } else {
                    // Clean typography: Immediate Milestone
                    if let Some(ref group) = immediate_group {
                        let header_color = match group.status {
                            EventStatus::Active => Color32::from_rgb(72, 239, 128),
                            EventStatus::Warning => Color32::from_rgb(251, 146, 60),
                            _ => Color32::from_rgb(245, 158, 11),
                        };

                        let target_str = format_time_mmss(group.target_time);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(target_str)
                                    .strong()
                                    .size(12.0)
                                    .color(header_color),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(2.0);
                                    if group.status == EventStatus::Active {
                                        ui.label(
                                            RichText::new("ACTIVO")
                                                .size(9.0)
                                                .strong()
                                                .color(Color32::from_rgb(72, 239, 128)),
                                        );
                                    } else if group.status == EventStatus::Warning {
                                        ui.label(
                                            RichText::new("PRONTO")
                                                .size(9.0)
                                                .strong()
                                                .color(Color32::from_rgb(251, 146, 60)),
                                        );
                                    }
                                },
                            );
                        });

                        for event in &group.events {
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!("• {}", event.kind.display_name()))
                                        .size(10.0)
                                        .color(Color32::from_rgb(226, 232, 240)),
                                );
                            });
                        }
                    } else {
                        ui.label(
                            RichText::new("Sin eventos pendientes")
                                .color(Color32::from_rgb(148, 163, 184))
                                .size(11.0),
                        );
                    }

                    // Subsequent Milestone Horizon Preview with independent bullet items (suppressed in minimal HUD mode)
                    if !self.config.minimal_hud {
                        if let Some(ref next) = next_group {
                            ui.add_space(3.0);
                            let nxt_target_str = format_time_mmss(next.target_time);

                            ui.label(
                                RichText::new(format!("Siguiente: {}", nxt_target_str))
                                    .size(9.5)
                                    .color(Color32::from_rgb(148, 163, 184)),
                            );
                            for event in &next.events {
                                ui.horizontal(|ui| {
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(format!("• {}", event.kind.display_name()))
                                            .size(9.5)
                                            .color(Color32::from_rgb(148, 163, 184)),
                                    );
                                });
                            }
                        }
                    }
                }
            }
        });

        // Persist window position only after an explicit user drag completes
        if self.was_dragged {
            let pointer_down = ctx.input(|i| i.pointer.primary_down());
            if !pointer_down {
                if let Some(outer) = ctx.input(|i| i.viewport().outer_rect) {
                    let cur_x = (outer.min.x * ppp).round() as i32;
                    let cur_y = (outer.min.y * ppp).round() as i32;
                    if cur_x > -10000 && cur_y > -10000 {
                        self.config.win_x = cur_x;
                        self.config.win_y = cur_y;
                        self.config.clamp_to_screen_with_size(self.screen_w, self.screen_h, phys_w, phys_h);
                        let _ = self.config.save_to_file(&self.config_path);
                    }
                }
                self.was_dragged = false;
            }
        }

        // Fast repaint in calibration/config mode; low CPU (200ms) in ghost mode
        if is_interactive || self.config_menu_open {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}
