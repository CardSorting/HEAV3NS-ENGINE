use crate::bridge::{
    AnimationSequencer, GameBridge, GameCommand, VisualBoardState, VisualEvent,
};
use crate::visuals::card_view::{
    compute_hand_arc_transform, draw_card_back, draw_card_visual, draw_rotated_rect,
    CardShaderMaterials, CardTransform, CardVisual, CARD_HEIGHT, CARD_WIDTH,
};
use crate::visuals::vfx::{
    CameraController, ProceduralAudioBank, VfxManager, VIRTUAL_HEIGHT, VIRTUAL_WIDTH,
};
use macroquad::prelude::*;

/// Stateful clickable corner prop on Layer 0 (Board & Environment) with cooldowns and particle triggers.
#[derive(Clone, Debug)]
pub struct CornerProp {
    pub index: usize,
    pub name: &'static str,
    pub subtitle: &'static str,
    pub rect: Rect,
    pub cooldown: f32,
    pub max_cooldown: f32,
    pub activation_count: u32,
    pub accent: Color,
}

impl CornerProp {
    pub fn trigger(&mut self, vfx: &mut VfxManager, audio: &ProceduralAudioBank) -> bool {
        if self.cooldown > 0.0 {
            return false;
        }
        self.cooldown = self.max_cooldown;
        self.activation_count += 1;
        let center = vec2(self.rect.x + self.rect.w * 0.5, self.rect.y + self.rect.h * 0.5);
        vfx.spawn_spell_impact(center, self.accent);
        vfx.spawn_mana_crystal_shatter(center, self.accent);
        vfx.spawn_floating_text(
            format!("{}!", self.name),
            center + vec2(0.0, -24.0),
            self.accent,
            22.0,
        );
        audio.play_chime();
        true
    }
}

/// Active targeting arrow state (Layer 3: bezier curve of animated chevron sprites).
#[derive(Clone, Debug)]
pub struct TargetingArrow {
    pub attacker_uid: i32,
    pub start_pos: Vec2,
    pub current_pos: Vec2,
    pub hovered_target_uid: Option<i32>,
}

/// Complete Puppet-Stage Client Renderer & Input Coordinator (`src/renderer.rs`).
pub struct TableStageRenderer {
    pub bridge: GameBridge,
    pub sequencer: AnimationSequencer,
    pub board_state: VisualBoardState,
    pub camera: CameraController,
    pub vfx: VfxManager,
    pub materials: CardShaderMaterials,
    pub audio: ProceduralAudioBank,
    pub hand_visuals: Vec<CardVisual>,
    pub board_visuals: [Vec<CardVisual>; 2],
    pub sequence_visuals: Vec<CardVisual>,
    pub corner_props: [CornerProp; 4],
    pub hovered_hand_uid: Option<i32>,
    pub dragged_card_uid: Option<i32>,
    pub drag_grab_offset: Vec2,
    pub targeting_arrow: Option<TargetingArrow>,
    pub prev_mouse_virtual: Vec2,
    pub cursor_velocity: Vec2,
    pub show_debug_overlay: bool,
    pub elapsed_time: f32,
    pub selected_prompt_uids: Vec<i32>,
}

impl TableStageRenderer {
    pub async fn new(seed: u64, starter: usize) -> Result<Self, String> {
        let (bridge, initial_events, initial_state) =
            GameBridge::new_with_interactive(seed, starter, true)?;
        let mut sequencer = AnimationSequencer::new();
        for ev in initial_events {
            sequencer.enqueue(ev, None);
        }
        let materials = CardShaderMaterials::load();
        let audio = ProceduralAudioBank::init().await;
        let vfx = VfxManager::new(600);
        let camera = CameraController::new();

        let corner_props = [
            CornerProp {
                index: 0,
                name: "Cheshire Lantern",
                subtitle: "Emits spectral violet sparks",
                rect: Rect::new(48.0, 44.0, 185.0, 96.0),
                cooldown: 0.0,
                max_cooldown: 1.5,
                activation_count: 0,
                accent: Color::new(0.75, 0.38, 0.96, 1.0),
            },
            CornerProp {
                index: 1,
                name: "Hatter's Chronometer",
                subtitle: "Chimes golden gears",
                rect: Rect::new(1687.0, 44.0, 185.0, 96.0),
                cooldown: 0.0,
                max_cooldown: 1.5,
                activation_count: 0,
                accent: Color::new(0.98, 0.80, 0.30, 1.0),
            },
            CornerProp {
                index: 2,
                name: "Queen's Rose Bush",
                subtitle: "Scatters crimson petals",
                rect: Rect::new(48.0, 816.0, 185.0, 96.0),
                cooldown: 0.0,
                max_cooldown: 1.5,
                activation_count: 0,
                accent: Color::new(0.95, 0.26, 0.38, 1.0),
            },
            CornerProp {
                index: 3,
                name: "Drink-Me Aludel",
                subtitle: "Bubbles azure alchemy",
                rect: Rect::new(1687.0, 816.0, 185.0, 96.0),
                cooldown: 0.0,
                max_cooldown: 1.5,
                activation_count: 0,
                accent: Color::new(0.26, 0.82, 0.98, 1.0),
            },
        ];

        let mut stage = Self {
            bridge,
            sequencer,
            board_state: initial_state,
            camera,
            vfx,
            materials,
            audio,
            hand_visuals: Vec::new(),
            board_visuals: [Vec::new(), Vec::new()],
            sequence_visuals: Vec::new(),
            corner_props,
            hovered_hand_uid: None,
            dragged_card_uid: None,
            drag_grab_offset: Vec2::ZERO,
            targeting_arrow: None,
            prev_mouse_virtual: vec2(960.0, 540.0),
            cursor_velocity: Vec2::ZERO,
            show_debug_overlay: true,
            elapsed_time: 0.0,
            selected_prompt_uids: Vec::new(),
        };
        stage.sync_card_visuals();
        Ok(stage)
    }

    /// Synchronizes `CardVisual` lists with `self.board_state` while preserving existing
    /// `current_transform` and velocity for smooth continuous spring damping.
    pub fn sync_card_visuals(&mut self) {
        // 1. Player Hand along quadratic Bezier arc
        let total_hand = self.board_state.player_hand.len();
        let mut next_hand = Vec::with_capacity(total_hand);
        for (idx, cdata) in self.board_state.player_hand.iter().enumerate() {
            let is_hov = self.hovered_hand_uid == Some(cdata.uid) && self.dragged_card_uid.is_none();
            let target = compute_hand_arc_transform(idx, total_hand, false, is_hov);
            if let Some(mut existing) = self
                .hand_visuals
                .iter()
                .find(|v| v.data.uid == cdata.uid)
                .cloned()
            {
                existing.data = cdata.clone();
                if self.dragged_card_uid != Some(cdata.uid) {
                    existing.target_transform = target;
                    existing.is_hovered = is_hov;
                    existing.is_dragging = false;
                }
                next_hand.push(existing);
            } else {
                // Spawn from Alice's deck position (bottom-right)
                let spawn_pos = vec2(1740.0, 740.0);
                next_hand.push(CardVisual::new(cdata.clone(), spawn_pos, target));
            }
        }
        self.hand_visuals = next_hand;

        // 2. Active Board Units (seat 0: bottom lane, seat 1: top lane)
        for seat in 0..2 {
            let units = &self.board_state.board_units[seat];
            let count = units.len();
            let lane_y = if seat == 0 { 615.0 } else { 320.0 };
            let mut next_units = Vec::with_capacity(count);
            for (idx, udata) in units.iter().enumerate() {
                let slot_pos = compute_board_slot_pos(idx, count, lane_y);
                let target = CardTransform {
                    position: slot_pos,
                    rotation: 0.0,
                    scale: vec2(0.76, 0.76),
                    tilt_25d: Vec2::ZERO,
                    z_order: idx as i32,
                };
                if let Some(mut existing) = self.board_visuals[seat]
                    .iter()
                    .find(|v| v.data.uid == udata.uid)
                    .cloned()
                {
                    existing.data = udata.clone();
                    existing.target_transform = target;
                    next_units.push(existing);
                } else {
                    let spawn = if seat == 0 {
                        vec2(960.0, 920.0)
                    } else {
                        vec2(960.0, 80.0)
                    };
                    next_units.push(CardVisual::new(udata.clone(), spawn, target));
                }
            }
            self.board_visuals[seat] = next_units;
        }

        // 3. Active Sequence Tray (center-left to center-right of the Enchanted Table)
        let seq_len = self.board_state.active_sequence.len();
        let mut next_seq = Vec::with_capacity(seq_len);
        for (idx, sdata) in self.board_state.active_sequence.iter().enumerate() {
            let slot_pos = compute_board_slot_pos(idx, seq_len, 468.0);
            let target = CardTransform {
                position: slot_pos,
                rotation: 0.0,
                scale: vec2(0.68, 0.68),
                tilt_25d: Vec2::ZERO,
                z_order: idx as i32,
            };
            if let Some(mut existing) = self
                .sequence_visuals
                .iter()
                .find(|v| v.data.uid == sdata.uid)
                .cloned()
            {
                existing.data = sdata.clone();
                existing.target_transform = target;
                next_seq.push(existing);
            } else {
                next_seq.push(CardVisual::new(sdata.clone(), vec2(960.0, 860.0), target));
            }
        }
        self.sequence_visuals = next_seq;
    }

    pub fn update(&mut self, dt: f32) {
        self.elapsed_time += dt;

        // Track virtual mouse position and cursor velocity
        let raw_mouse = Vec2::from(mouse_position());
        let mouse_v = self.camera.screen_to_virtual(raw_mouse);
        let inst_vel = (mouse_v - self.prev_mouse_virtual) / dt.max(0.0001);
        self.cursor_velocity = self.cursor_velocity.lerp(inst_vel, 0.35);
        self.prev_mouse_virtual = mouse_v;

        // Toggle Debug Overlay with F3 or Tab
        if is_key_pressed(KeyCode::F3) || is_key_pressed(KeyCode::Tab) {
            self.show_debug_overlay = !self.show_debug_overlay;
        }
        // Quick Reset with R
        if is_key_pressed(KeyCode::R) {
            self.submit_command(GameCommand::ResetMatch {
                seed: self.board_state.seed + 1,
                starter: 0,
            });
        }
        // Spacebar resolves open sequence or ends turn (or selects first option if modal prompt is open)
        if is_key_pressed(KeyCode::Space) && !self.sequencer.is_busy() {
            if let Some(ref prompt) = self.board_state.prompt {
                if !prompt.options.is_empty() {
                    self.selected_prompt_uids.clear();
                    let uids = if prompt.required_cards > 1 {
                        Vec::new()
                    } else {
                        prompt.options[0].target_uid.into_iter().collect()
                    };
                    self.submit_command(GameCommand::SubmitDecision {
                        option_index: Some(0),
                        selected_uids: uids,
                    });
                }
            } else if self.board_state.phase == "placement" && !self.board_state.active_sequence.is_empty() {
                self.submit_command(GameCommand::ResolveSequence);
            } else {
                self.submit_command(GameCommand::EndTurn);
            }
        }

        // Update corner prop cooldowns
        for prop in &mut self.corner_props {
            if prop.cooldown > 0.0 {
                prop.cooldown = (prop.cooldown - dt).max(0.0);
            }
        }

        // Advance AnimationSequencer and trigger VFX/Camera/Audio for popped events
        let started_events = self.sequencer.update(dt, &mut self.board_state);
        for ev in started_events {
            self.trigger_vfx_for_event(&ev);
        }
        self.sync_card_visuals();

        // Process Mouse Picks, Hover, Drag-and-Drop, Targeting Arrow, and Button Clicks
        self.handle_mouse_input(mouse_v);

        // Update all CardVisual spring/tween transforms & idle breathing animations
        for card in &mut self.hand_visuals {
            card.update(dt, self.cursor_velocity);
        }
        for seat in 0..2 {
            for (idx, unit) in self.board_visuals[seat].iter_mut().enumerate() {
                // Subtle idle breathing animation (sine-wave scale & vertical translation)
                let phase = self.elapsed_time * 2.4 + (idx as f32) * 1.1 + (seat as f32) * 2.0;
                unit.target_transform.position.y += phase.sin() * 0.25;
                let breath = 0.76 + phase.sin() * 0.018;
                unit.target_transform.scale = vec2(breath, breath);
                unit.update(dt, Vec2::ZERO);
            }
        }
        for seq_c in &mut self.sequence_visuals {
            seq_c.update(dt, Vec2::ZERO);
        }

        self.camera.update(dt);
        self.vfx.update(dt);
    }

    fn trigger_vfx_for_event(&mut self, ev: &VisualEvent) {
        match ev {
            VisualEvent::TurnStarted { seat, turn_number } => {
                let who = if *seat == 0 {
                    "ALICE'S TURN"
                } else {
                    "WHITE RABBIT'S TURN"
                };
                let accent = if *seat == 0 {
                    Color::new(0.95, 0.78, 0.35, 1.0)
                } else {
                    Color::new(0.92, 0.35, 0.48, 1.0)
                };
                self.vfx
                    .show_banner(who, format!("Turn {turn_number} - 6 Actions Refreshed"), accent);
                self.audio.play_chime();
            }
            VisualEvent::CardDrawn { seat, title, .. } => {
                let pos = if *seat == 0 {
                    vec2(960.0, 880.0)
                } else {
                    vec2(960.0, 120.0)
                };
                if *seat == 0 {
                    self.vfx.spawn_floating_text(
                        format!("+ {title}"),
                        pos,
                        Color::new(0.55, 0.88, 1.0, 1.0),
                        22.0,
                    );
                }
                self.audio.play_hover();
            }
            VisualEvent::ManaSpent {
                seat,
                amount,
                remaining,
            } => {
                let crystal_pos = if *seat == 0 {
                    vec2(350.0 + (*remaining as f32) * 32.0, 770.0)
                } else {
                    vec2(350.0 + (*remaining as f32) * 32.0, 175.0)
                };
                self.vfx.spawn_mana_crystal_shatter(
                    crystal_pos,
                    Color::new(0.24, 0.68, 1.0, 0.95),
                );
                self.vfx.spawn_floating_text(
                    format!("-{amount} Action{}", if *amount == 1 { "" } else { "s" }),
                    crystal_pos + vec2(0.0, -18.0),
                    Color::new(0.38, 0.78, 1.0, 1.0),
                    21.0,
                );
            }
            VisualEvent::UnitSummoned {
                seat,
                title,
                cost,
                slot_index,
                ..
            } => {
                let lane_y = if *seat == 0 { 615.0 } else { 320.0 };
                let count = self.board_state.board_units[*seat].len().max(1);
                let pos = compute_board_slot_pos(*slot_index, count, lane_y);
                self.vfx.spawn_minion_dust_cloud(pos);
                let shake_intensity = (0.28 + (*cost as f32) * 0.14).clamp(0.3, 0.85);
                self.camera.trigger_shake(shake_intensity, 0.35);
                self.vfx.spawn_floating_text(
                    format!("Summoned {title}"),
                    pos + vec2(0.0, -45.0),
                    Color::new(0.98, 0.86, 0.45, 1.0),
                    24.0,
                );
                self.audio.play_impact();
            }
            VisualEvent::SpellCast {
                seat,
                title,
                target_uid,
                ..
            } => {
                let start = if *seat == 0 {
                    vec2(960.0, 720.0)
                } else {
                    vec2(960.0, 220.0)
                };
                let end = target_uid
                    .and_then(|u| self.find_unit_position(u))
                    .unwrap_or_else(|| {
                        if *seat == 0 {
                            vec2(960.0, 260.0)
                        } else {
                            vec2(960.0, 680.0)
                        }
                    });
                let col = if *seat == 0 {
                    Color::new(0.32, 0.86, 1.0, 1.0)
                } else {
                    Color::new(1.0, 0.36, 0.52, 1.0)
                };
                self.vfx.spawn_projectile(start, end, col, title);
                self.vfx.spawn_floating_text(
                    format!("Cast {title}!"),
                    start + vec2(0.0, -30.0),
                    col,
                    24.0,
                );
                self.audio.play_card();
            }
            VisualEvent::DamageDealt {
                target_seat,
                target_uid,
                amount,
                is_shield_break,
                ..
            } => {
                let start = if *target_seat == 1 {
                    vec2(960.0, 600.0)
                } else {
                    vec2(960.0, 320.0)
                };
                let target_pos = target_uid
                    .and_then(|u| self.find_unit_position(u))
                    .unwrap_or_else(|| {
                        if *target_seat == 1 {
                            vec2(960.0, 185.0)
                        } else {
                            vec2(960.0, 755.0)
                        }
                    });
                self.vfx.spawn_projectile(
                    start,
                    target_pos,
                    Color::new(1.0, 0.42, 0.22, 1.0),
                    "Strike",
                );
                self.vfx.spawn_mana_crystal_shatter(
                    target_pos,
                    Color::new(1.0, 0.35, 0.25, 1.0),
                );
                self.camera.trigger_shake(
                    if *is_shield_break { 0.72 } else { 0.45 },
                    0.42,
                );
                let label = if *is_shield_break {
                    format!("-{amount} SHIELD BROKEN!")
                } else {
                    format!("-{amount} DMG")
                };
                self.vfx.spawn_floating_text(
                    label,
                    target_pos + vec2(0.0, -25.0),
                    Color::new(1.0, 0.28, 0.28, 1.0),
                    30.0,
                );
                self.audio.play_shatter();
            }
            VisualEvent::ShieldRevealed {
                seat,
                title,
                trigger_text,
                ..
            } => {
                let pos = if *seat == 1 {
                    vec2(960.0, 220.0)
                } else {
                    vec2(960.0, 720.0)
                };
                let msg = if let Some(t) = trigger_text {
                    format!("Shield Trigger: {title} ({t})")
                } else {
                    format!("Shield Revealed: {title}")
                };
                self.vfx.spawn_floating_text(
                    msg,
                    pos,
                    Color::new(1.0, 0.86, 0.32, 1.0),
                    24.0,
                );
            }
            VisualEvent::UnitBounced { seat, title, .. } => {
                let pos = if *seat == 0 {
                    vec2(960.0, 615.0)
                } else {
                    vec2(960.0, 320.0)
                };
                self.vfx.spawn_spell_impact(pos, Color::new(0.45, 0.85, 1.0, 1.0));
                self.vfx.spawn_floating_text(
                    format!("Bounced {title} to Hand!"),
                    pos,
                    Color::new(0.45, 0.88, 1.0, 1.0),
                    24.0,
                );
                self.audio.play_card();
            }
            VisualEvent::EffectCancelled { title, kind, .. } => {
                let pos = vec2(960.0, 470.0);
                self.vfx.spawn_spell_impact(pos, Color::new(1.0, 0.25, 0.45, 1.0));
                self.camera.trigger_shake(0.50, 0.30);
                self.vfx.spawn_floating_text(
                    format!("COUNTERED: {title} ({kind})"),
                    pos,
                    Color::new(1.0, 0.32, 0.48, 1.0),
                    28.0,
                );
                self.audio.play_shatter();
            }
            VisualEvent::MatchWon { winner_seat } => {
                let who = if *winner_seat == 0 {
                    "VICTORY - ALICE TRIUMPHS!"
                } else {
                    "DEFEAT - WHITE RABBIT PREVAILS"
                };
                self.vfx.show_banner(
                    who,
                    "Click 'New Match' or press [R] to play again",
                    Color::new(1.0, 0.84, 0.28, 1.0),
                );
                self.camera.trigger_shake(0.85, 0.65);
                self.audio.play_chime();
            }
            _ => {}
        }
    }

    fn find_unit_position(&self, uid: i32) -> Option<Vec2> {
        for seat in 0..2 {
            for u in &self.board_visuals[seat] {
                if u.data.uid == uid {
                    return Some(u.current_transform.position);
                }
            }
        }
        None
    }

    fn handle_mouse_input(&mut self, mouse_v: Vec2) {
        // 0. If an interactive decision prompt modal is open on Layer 4, handle option clicks first!
        if let Some(prompt) = self.board_state.prompt.clone() {
            if is_mouse_button_pressed(MouseButton::Left) {
                let modal_w = 780.0;
                let opt_h = 44.0;
                let modal_h = (130.0 + (prompt.options.len() as f32) * (opt_h + 10.0)).min(620.0);
                let modal_x = 960.0 - modal_w * 0.5;
                let modal_y = 470.0 - modal_h * 0.5;

                for (i, opt) in prompt.options.iter().enumerate() {
                    let btn_rect = Rect::new(
                        modal_x + 28.0,
                        modal_y + 92.0 + (i as f32) * (opt_h + 10.0),
                        modal_w - 56.0,
                        opt_h,
                    );
                    if btn_rect.contains(mouse_v) {
                        self.audio.play_card();
                        if prompt.required_cards > 1 {
                            if let Some(u) = opt.target_uid {
                                if let Some(pos) =
                                    self.selected_prompt_uids.iter().position(|x| *x == u)
                                {
                                    self.selected_prompt_uids.remove(pos);
                                } else {
                                    self.selected_prompt_uids.push(u);
                                }
                                if self.selected_prompt_uids.len() >= prompt.required_cards {
                                    let chosen = std::mem::take(&mut self.selected_prompt_uids);
                                    self.submit_command(GameCommand::SubmitDecision {
                                        option_index: None,
                                        selected_uids: chosen,
                                    });
                                }
                            }
                        } else {
                            self.selected_prompt_uids.clear();
                            let uids = opt.target_uid.into_iter().collect();
                            self.submit_command(GameCommand::SubmitDecision {
                                option_index: Some(opt.index),
                                selected_uids: uids,
                            });
                        }
                        return;
                    }
                }
            }
            return;
        } else {
            self.selected_prompt_uids.clear();
        }

        // 1. Update hovered card in player's hand (check in reverse Z-order so top-most wins)
        let prev_hover = self.hovered_hand_uid;
        if self.dragged_card_uid.is_none() && self.targeting_arrow.is_none() {
            let mut found = None;
            let mut best_z = i32::MIN;
            for card in &self.hand_visuals {
                if card.contains_point(mouse_v) && card.current_transform.z_order >= best_z {
                    best_z = card.current_transform.z_order;
                    found = Some(card.data.uid);
                }
            }
            self.hovered_hand_uid = found;
            if self.hovered_hand_uid.is_some() && self.hovered_hand_uid != prev_hover {
                self.audio.play_hover();
            }
        }

        // 2. Handle active card dragging
        if let Some(drag_uid) = self.dragged_card_uid {
            if is_mouse_button_down(MouseButton::Left) {
                if let Some(card) = self
                    .hand_visuals
                    .iter_mut()
                    .find(|c| c.data.uid == drag_uid)
                {
                    card.is_dragging = true;
                    card.is_hovered = false;
                    card.target_transform.position = mouse_v + self.drag_grab_offset;
                    card.target_transform.scale = vec2(1.12, 1.12);
                    card.target_transform.z_order = 200;
                }
            } else {
                // Mouse released: check if dropped onto the Enchanted Table battlefield (y < 765.0)
                let dropped_on_table = mouse_v.y < 765.0
                    && mouse_v.x > 240.0
                    && mouse_v.x < 1680.0;
                let target_uid = self.find_unit_at_point(mouse_v);
                self.dragged_card_uid = None;

                if dropped_on_table {
                    self.submit_command(GameCommand::PlayCard {
                        card_id: drag_uid,
                        target_id: target_uid,
                    });
                } else {
                    self.sync_card_visuals();
                }
            }
            return;
        }

        // 3. Handle active targeting arrow (dragging from a friendly board unit to an enemy unit or shield row)
        if self.targeting_arrow.is_some() {
            let hovered_target = self.find_unit_at_point(mouse_v);
            if is_mouse_button_down(MouseButton::Left) {
                if let Some(ref mut arrow) = self.targeting_arrow {
                    arrow.current_pos = mouse_v;
                    arrow.hovered_target_uid = hovered_target;
                }
            } else if let Some(arrow) = self.targeting_arrow.take() {
                let attacker_uid = arrow.attacker_uid;
                let defender_uid = arrow.hovered_target_uid.unwrap_or(-1);
                let valid_release = arrow.current_pos.distance(arrow.start_pos) > 45.0;
                if valid_release {
                    self.submit_command(GameCommand::Attack {
                        attacker_id: attacker_uid,
                        defender_id: defender_uid,
                    });
                }
            }
            return;
        }

        // 4. Handle initial Left Mouse Button press
        if is_mouse_button_pressed(MouseButton::Left) {
            // 4a. Check interactive Corner Props (Layer 0)
            for idx in 0..self.corner_props.len() {
                if self.corner_props[idx].rect.contains(mouse_v) {
                    if self.corner_props[idx].trigger(&mut self.vfx, &self.audio) {
                        self.submit_command(GameCommand::TriggerProp { prop_index: idx });
                    }
                    return;
                }
            }

            // 4b. Check UI Action Buttons (Resolve Sequence, End Turn, New Match)
            let resolve_rect = Rect::new(1688.0, 445.0, 184.0, 56.0);
            let end_turn_rect = Rect::new(1688.0, 515.0, 184.0, 56.0);
            let reset_rect = Rect::new(1688.0, 585.0, 184.0, 44.0);

            if resolve_rect.contains(mouse_v) {
                if self.board_state.phase == "placement" {
                    self.submit_command(GameCommand::ResolveSequence);
                } else if self.board_state.phase == "resolved" {
                    self.submit_command(GameCommand::BeginSequence);
                }
                return;
            }
            if end_turn_rect.contains(mouse_v) {
                self.submit_command(GameCommand::EndTurn);
                return;
            }
            if reset_rect.contains(mouse_v) {
                self.submit_command(GameCommand::ResetMatch {
                    seed: self.board_state.seed + 1,
                    starter: 0,
                });
                return;
            }

            // 4c. Start dragging hovered hand card
            if let Some(hov_uid) = self.hovered_hand_uid {
                if let Some(card) = self.hand_visuals.iter().find(|c| c.data.uid == hov_uid) {
                    self.dragged_card_uid = Some(hov_uid);
                    self.drag_grab_offset = card.current_transform.position - mouse_v;
                    self.audio.play_hover();
                    return;
                }
            }

            // 4d. Start dragging a targeting arrow from a friendly unit on the board (Layer 1 -> Layer 3)
            for unit in &self.board_visuals[0] {
                if unit.contains_point(mouse_v) {
                    self.targeting_arrow = Some(TargetingArrow {
                        attacker_uid: unit.data.uid,
                        start_pos: unit.current_transform.position,
                        current_pos: mouse_v,
                        hovered_target_uid: None,
                    });
                    self.audio.play_hover();
                    return;
                }
            }
        }
    }

    fn find_unit_at_point(&self, pt: Vec2) -> Option<i32> {
        for seat in (0..2).rev() {
            for unit in &self.board_visuals[seat] {
                if unit.contains_point(pt) {
                    return Some(unit.data.uid);
                }
            }
        }
        None
    }

    pub fn submit_command(&mut self, cmd: GameCommand) {
        match self.bridge.dispatch(cmd) {
            Ok((events, final_state)) => {
                let ev_vec: Vec<VisualEvent> = events.into_iter().collect();
                self.sequencer.enqueue_batch(ev_vec, final_state);
            }
            Err(err_msg) => {
                self.vfx.spawn_floating_text(
                    err_msg,
                    vec2(960.0, 740.0),
                    Color::new(1.0, 0.35, 0.35, 1.0),
                    24.0,
                );
                self.camera.trigger_shake(0.25, 0.20);
            }
        }
    }

    /// Executes the explicit 5-Layer Z-Sorted Render Pipeline inside the main `#[macroquad::main]` loop:
    /// - Layer 0: Board & Environment (framed "box" battlefield & interactive corner props)
    /// - Layer 1: Board Slots & Minions (active units, health/attack badges, taunt/shield frames, breathing)
    /// - Layer 2: Player & Opponent Hands (dynamic bezier arc, hover 1.3x magnification & vertical shift)
    /// - Layer 3: Active Dragging & Projectiles (dragged card, bezier chevron targeting arrow, spell projectiles)
    /// - Layer 4: VFX & Screen Space UI (floating combat text, particle bursts, turn banners, debug overlay)
    pub fn render(&self) {
        clear_background(Color::new(0.05, 0.03, 0.07, 1.0));

        let cam = self.camera.to_camera_2d();
        set_camera(&cam);

        self.draw_layer_0_board_and_environment();
        self.draw_layer_1_board_slots_and_minions();
        self.draw_layer_2_player_and_opponent_hands();
        self.draw_layer_3_active_dragging_and_projectiles();
        self.draw_layer_4_vfx_and_screen_space_ui();

        set_default_camera();
    }

    /// Layer 0 (Board & Environment): Framed "box" battlefield background, mana trays, shield rows,
    /// and interactive stateful corner props with cooldowns and particle triggers.
    fn draw_layer_0_board_and_environment(&self) {
        // Outer Enchanted Tavern / Velvet Table backdrop
        draw_rectangle(
            0.0,
            0.0,
            VIRTUAL_WIDTH,
            VIRTUAL_HEIGHT,
            Color::new(0.09, 0.05, 0.11, 1.0),
        );

        // Framed "Box" Battlefield Stage
        let box_rect = Rect::new(248.0, 142.0, 1424.0, 650.0);
        // Outer gilded brass frame
        draw_rectangle(
            box_rect.x - 10.0,
            box_rect.y - 10.0,
            box_rect.w + 20.0,
            box_rect.h + 20.0,
            Color::new(0.46, 0.34, 0.20, 1.0),
        );
        draw_rectangle(
            box_rect.x,
            box_rect.y,
            box_rect.w,
            box_rect.h,
            Color::new(0.15, 0.10, 0.18, 1.0),
        );
        // Inner felt playmat halves (Opponent upper crimson-violet, Alice lower royal-plum)
        draw_rectangle(
            box_rect.x + 14.0,
            box_rect.y + 14.0,
            box_rect.w - 28.0,
            (box_rect.h - 28.0) * 0.48,
            Color::new(0.19, 0.11, 0.19, 1.0),
        );
        draw_rectangle(
            box_rect.x + 14.0,
            box_rect.y + 14.0 + (box_rect.h - 28.0) * 0.48,
            box_rect.w - 28.0,
            (box_rect.h - 28.0) * 0.52,
            Color::new(0.14, 0.12, 0.22, 1.0),
        );

        // Center Enchanted Sequence Divider Line
        let mid_y = 468.0;
        draw_line(
            box_rect.x + 20.0,
            mid_y,
            box_rect.x + box_rect.w - 20.0,
            mid_y,
            3.0,
            Color::new(0.76, 0.62, 0.34, 0.55),
        );

        // Shield Rows & Mana Crystal Trays for White Rabbit (seat 1) and Alice (seat 0)
        self.draw_player_hud_strip(1, 162.0);
        self.draw_player_hud_strip(0, 742.0);

        // Interactive Corner Props (4 stateful clickable props with cooldown bars)
        for prop in &self.corner_props {
            let ready = prop.cooldown <= 0.001;
            let bg = if ready {
                Color::new(0.18, 0.12, 0.24, 0.95)
            } else {
                Color::new(0.11, 0.08, 0.14, 0.80)
            };
            draw_rectangle(prop.rect.x, prop.rect.y, prop.rect.w, prop.rect.h, bg);
            draw_rectangle_lines(
                prop.rect.x,
                prop.rect.y,
                prop.rect.w,
                prop.rect.h,
                2.5,
                if ready {
                    prop.accent
                } else {
                    Color::new(0.4, 0.4, 0.45, 0.7)
                },
            );

            // Animated prop emblem
            let orb_x = prop.rect.x + 26.0;
            let orb_y = prop.rect.y + 34.0;
            let pulse = if ready {
                1.0 + 0.12 * (self.elapsed_time * 3.5 + prop.index as f32).sin()
            } else {
                0.75
            };
            draw_circle(orb_x, orb_y, 13.0 * pulse, prop.accent);
            draw_text(
                prop.name,
                prop.rect.x + 46.0,
                prop.rect.y + 30.0,
                16.0,
                Color::new(0.96, 0.92, 0.82, 1.0),
            );
            draw_text(
                prop.subtitle,
                prop.rect.x + 46.0,
                prop.rect.y + 50.0,
                12.5,
                Color::new(0.75, 0.70, 0.80, 1.0),
            );

            // Cooldown / Activation status bar
            let bar_x = prop.rect.x + 12.0;
            let bar_y = prop.rect.y + prop.rect.h - 18.0;
            let bar_w = prop.rect.w - 24.0;
            draw_rectangle(bar_x, bar_y, bar_w, 8.0, Color::new(0.06, 0.04, 0.08, 1.0));
            let fill = if ready {
                1.0
            } else {
                1.0 - (prop.cooldown / prop.max_cooldown).clamp(0.0, 1.0)
            };
            draw_rectangle(bar_x, bar_y, bar_w * fill, 8.0, prop.accent);
        }
    }

    fn draw_player_hud_strip(&self, seat: usize, y: f32) {
        let name = if seat == 0 {
            "ALICE (HUMAN)"
        } else {
            "WHITE RABBIT (AUDIT AI)"
        };
        let is_active = self.board_state.active_seat == seat;
        let badge_col = if is_active {
            Color::new(0.95, 0.78, 0.34, 1.0)
        } else {
            Color::new(0.58, 0.52, 0.64, 1.0)
        };
        draw_text(name, 272.0, y + 8.0, 20.0, badge_col);

        // Mana / Action Crystals (up to 6)
        let cur_mana = self.board_state.mana[seat];
        let max_mana = self.board_state.max_mana[seat];
        for i in 0..max_mana {
            let cx = 505.0 + (i as f32) * 28.0;
            let cy = y + 2.0;
            let filled = i < cur_mana;
            draw_poly(
                cx,
                cy,
                6,
                10.5,
                30.0,
                if filled {
                    Color::new(0.22, 0.72, 1.0, 1.0)
                } else {
                    Color::new(0.18, 0.20, 0.28, 0.85)
                },
            );
            draw_poly_lines(
                cx,
                cy,
                6,
                11.0,
                30.0,
                1.8,
                Color::new(0.75, 0.88, 1.0, 0.85),
            );
        }
        draw_text(
            format!("{cur_mana}/{max_mana} Actions"),
            682.0,
            y + 8.0,
            18.0,
            Color::new(0.72, 0.88, 1.0, 1.0),
        );

        // Shields Row (miniature shield emblems)
        let shields = self.board_state.shields_count[seat];
        draw_text(
            format!("Shields ({shields}):"),
            855.0,
            y + 8.0,
            18.0,
            Color::new(0.96, 0.84, 0.48, 1.0),
        );
        for s in 0..shields {
            let sx = 985.0 + (s as f32) * 34.0;
            let sy = y - 10.0;
            draw_rectangle(
                sx,
                sy,
                24.0,
                28.0,
                Color::new(0.78, 0.60, 0.26, 0.95),
            );
            draw_rectangle_lines(
                sx,
                sy,
                24.0,
                28.0,
                2.0,
                Color::new(1.0, 0.90, 0.58, 1.0),
            );
        }

        // Deck & Discard Counters
        let deck_c = self.board_state.deck_count[seat];
        let disc_c = self.board_state.discard_count[seat];
        draw_text(
            format!("Deck: {deck_c}  |  Discard: {disc_c}"),
            1395.0,
            y + 8.0,
            18.0,
            Color::new(0.80, 0.76, 0.86, 1.0),
        );
    }

    /// Layer 1 (Board Slots & Minions): Active units on the board with health/attack badges,
    /// taunt/shield frames, sequence slots, and subtle idle breathing animations.
    fn draw_layer_1_board_slots_and_minions(&self) {
        // Draw 5 subtle board slot frames per lane
        for seat in 0..2 {
            let lane_y = if seat == 0 { 615.0 } else { 320.0 };
            for slot in 0..5 {
                let pos = compute_board_slot_pos(slot, 5, lane_y);
                draw_rectangle_lines(
                    pos.x - 62.0,
                    pos.y - 88.0,
                    124.0,
                    176.0,
                    1.5,
                    Color::new(0.45, 0.36, 0.54, 0.26),
                );
            }
        }

        // Draw active sequence banner & cards in center strip (y = 468.0)
        if !self.sequence_visuals.is_empty() {
            draw_rectangle(
                420.0,
                384.0,
                1080.0,
                168.0,
                Color::new(0.12, 0.08, 0.18, 0.72),
            );
            draw_rectangle_lines(
                420.0,
                384.0,
                1080.0,
                168.0,
                2.0,
                Color::new(0.95, 0.78, 0.35, 0.85),
            );
            draw_text(
                "ACTIVE SEQUENCE (Left -> Right)",
                436.0,
                406.0,
                17.0,
                Color::new(0.98, 0.86, 0.46, 1.0),
            );
            for seq_c in &self.sequence_visuals {
                draw_card_visual(seq_c, &self.materials, self.elapsed_time, false);
            }
        }

        // Draw active units on both lanes with Taunt / Shield frames
        let arrow_target = self
            .targeting_arrow
            .as_ref()
            .and_then(|a| a.hovered_target_uid);

        for seat in 0..2 {
            for unit in &self.board_visuals[seat] {
                let pos = unit.current_transform.position;
                let sc = unit.current_transform.scale;
                let w = CARD_WIDTH * sc.x;
                let h = CARD_HEIGHT * sc.y;

                // Taunt heavy iron/gold crest frame
                if unit.data.has_taunt {
                    draw_poly(
                        pos.x,
                        pos.y,
                        8,
                        w * 0.64,
                        22.5,
                        Color::new(0.76, 0.62, 0.28, 0.42),
                    );
                    draw_poly_lines(
                        pos.x,
                        pos.y,
                        8,
                        w * 0.65,
                        22.5,
                        3.0,
                        Color::new(0.98, 0.84, 0.38, 0.90),
                    );
                }

                let is_targeted = arrow_target == Some(unit.data.uid);
                draw_card_visual(unit, &self.materials, self.elapsed_time, is_targeted);

                // Divine / Trigger Aegis bubble overlay if unit has_shield
                if unit.data.has_shield {
                    let bubble_alpha = 0.22 + 0.10 * (self.elapsed_time * 3.0 + unit.pulse_phase).sin();
                    draw_Rectangle_shield_aura(pos, vec2(w + 10.0, h + 10.0), bubble_alpha);
                }
            }
        }
    }

    /// Layer 2 (Player & Opponent Hands): Renders opponent hand arc and player hand arc.
    /// Hovered cards snap to the top of the hand layer (`z_order = 100`), scale up (`1.3x`),
    /// and shift vertically so the full card text is readable.
    fn draw_layer_2_player_and_opponent_hands(&self) {
        // 1. Opponent Hand (face-down arc along top edge)
        let opp_count = self.board_state.opponent_hand_count;
        for i in 0..opp_count {
            let tf = compute_hand_arc_transform(i, opp_count, true, false);
            draw_card_back(&tf, &self.materials, WHITE);
        }

        // 2. Player Hand sorted by Z-order (excluding any actively dragged card, which draws on Layer 3)
        let mut sorted_indices: Vec<usize> = (0..self.hand_visuals.len())
            .filter(|&i| Some(self.hand_visuals[i].data.uid) != self.dragged_card_uid)
            .collect();
        sorted_indices.sort_by_key(|&i| self.hand_visuals[i].current_transform.z_order);

        for idx in sorted_indices {
            let card = &self.hand_visuals[idx];
            draw_card_visual(card, &self.materials, self.elapsed_time, false);
        }
    }

    /// Layer 3 (Active Dragging & Projectiles): Renders the currently dragged card with velocity tilt,
    /// the bezier targeting arrow of animated chevron sprites, and active spell projectiles.
    fn draw_layer_3_active_dragging_and_projectiles(&self) {
        // 1. Dragged Card
        if let Some(drag_uid) = self.dragged_card_uid {
            if let Some(card) = self.hand_visuals.iter().find(|c| c.data.uid == drag_uid) {
                draw_card_visual(card, &self.materials, self.elapsed_time, true);
            }
        }

        // 2. Targeting Arrow (Bezier curve of animated chevron sprites)
        if let Some(ref arrow) = self.targeting_arrow {
            self.draw_bezier_chevron_arrow(arrow);
        }

        // 3. Active Spell Projectiles
        self.vfx.draw_layer3_projectiles();
    }

    fn draw_bezier_chevron_arrow(&self, arrow: &TargetingArrow) {
        let p0 = arrow.start_pos;
        let p2 = arrow.current_pos;
        let mid = (p0 + p2) * 0.5 + vec2(0.0, -130.0);

        let num_chevrons = 16;
        let scroll = (self.elapsed_time * 2.2).fract();
        let col = if arrow.hovered_target_uid.is_some() {
            Color::new(1.0, 0.28, 0.32, 0.95)
        } else {
            Color::new(1.0, 0.78, 0.28, 0.90)
        };

        for i in 0..num_chevrons {
            let t = ((i as f32 + scroll) / num_chevrons as f32).clamp(0.02, 0.98);
            let om = 1.0 - t;
            let pos = p0 * (om * om) + mid * (2.0 * om * t) + p2 * (t * t);
            // Derivative B'(t) = 2(1-t)(P1 - P0) + 2t(P2 - P1)
            let tangent = ((mid - p0) * (2.0 * om) + (p2 - mid) * (2.0 * t)).normalize_or_zero();
            let perp = vec2(-tangent.y, tangent.x);
            let size = 12.0 + t * 8.0;

            let tip = pos + tangent * size;
            let left = pos - tangent * (size * 0.6) + perp * (size * 0.75);
            let right = pos - tangent * (size * 0.6) - perp * (size * 0.75);

            draw_triangle(tip, left, right, col);
        }

        // Target reticle at cursor tip
        draw_circle_lines(p2.x, p2.y, 24.0, 3.5, col);
        draw_circle(p2.x, p2.y, 5.0, WHITE);
    }

    /// Layer 4 (VFX & Screen Space UI): Renders particle bursts, floating combat text, turn-change
    /// banners, action control buttons, combat log sidebar, interactive modal prompts, and debug overlay.
    fn draw_layer_4_vfx_and_screen_space_ui(&self) {
        // 1. Action Control Buttons (right side of battlefield)
        let can_act = self.board_state.winner.is_none()
            && self.board_state.active_seat == 0
            && !self.sequencer.is_busy()
            && self.board_state.prompt.is_none();

        let resolve_label = if self.board_state.phase == "placement" {
            "Resolve Sequence"
        } else {
            "New Sequence"
        };
        draw_ui_button(
            Rect::new(1688.0, 445.0, 184.0, 56.0),
            resolve_label,
            "[Space]",
            can_act,
            Color::new(0.22, 0.64, 0.44, 1.0),
        );
        draw_ui_button(
            Rect::new(1688.0, 515.0, 184.0, 56.0),
            "End Turn",
            "Pass to Rabbit",
            can_act,
            Color::new(0.76, 0.48, 0.22, 1.0),
        );
        draw_ui_button(
            Rect::new(1688.0, 585.0, 184.0, 44.0),
            "New Match",
            "[R] Next Seed",
            true,
            Color::new(0.36, 0.28, 0.48, 1.0),
        );

        // 2. Left Sidebar: Recent Combat Log & Turn Info
        let log_box = Rect::new(48.0, 165.0, 185.0, 625.0);
        draw_rectangle(
            log_box.x,
            log_box.y,
            log_box.w,
            log_box.h,
            Color::new(0.10, 0.07, 0.14, 0.90),
        );
        draw_rectangle_lines(
            log_box.x,
            log_box.y,
            log_box.w,
            log_box.h,
            2.0,
            Color::new(0.55, 0.44, 0.30, 0.80),
        );
        draw_text(
            format!("TURN {} - {}", self.board_state.turn_number, self.board_state.phase.to_uppercase()),
            log_box.x + 12.0,
            log_box.y + 26.0,
            16.0,
            Color::new(0.96, 0.84, 0.48, 1.0),
        );
        draw_text(
            "CHRONICLE",
            log_box.x + 12.0,
            log_box.y + 48.0,
            14.0,
            Color::new(0.70, 0.64, 0.78, 1.0),
        );

        let mut log_y = log_box.y + 74.0;
        for entry in self.board_state.combat_log.iter().rev().take(16) {
            let short = if entry.chars().count() > 24 {
                let s: String = entry.chars().take(21).collect();
                format!("{s}...")
            } else {
                entry.clone()
            };
            draw_text(
                &short,
                log_box.x + 10.0,
                log_y,
                13.5,
                Color::new(0.86, 0.82, 0.90, 0.92),
            );
            log_y += 32.0;
        }

        // 3. Status Instruction Bar above player's hand
        draw_rectangle(
            420.0,
            802.0,
            1080.0,
            34.0,
            Color::new(0.08, 0.05, 0.12, 0.88),
        );
        let st_dims = measure_text(&self.board_state.status_line, None, 19, 1.0);
        draw_text(
            &self.board_state.status_line,
            960.0 - st_dims.width * 0.5,
            825.0,
            19.0,
            Color::new(0.96, 0.90, 0.74, 1.0),
        );

        // 4. Interactive Modal Decision Overlay (if a choice, Counter response, pick, or discard is active)
        if let Some(ref prompt) = self.board_state.prompt {
            draw_rectangle(
                0.0,
                0.0,
                VIRTUAL_WIDTH,
                VIRTUAL_HEIGHT,
                Color::new(0.02, 0.01, 0.04, 0.65),
            );
            let modal_w = 780.0;
            let opt_h = 44.0;
            let modal_h = (130.0 + (prompt.options.len() as f32) * (opt_h + 10.0)).min(620.0);
            let modal_x = 960.0 - modal_w * 0.5;
            let modal_y = 470.0 - modal_h * 0.5;

            draw_rectangle(
                modal_x,
                modal_y,
                modal_w,
                modal_h,
                Color::new(0.11, 0.07, 0.16, 0.97),
            );
            draw_rectangle_lines(
                modal_x,
                modal_y,
                modal_w,
                modal_h,
                3.0,
                Color::new(0.95, 0.78, 0.35, 1.0),
            );

            let td = measure_text(&prompt.title, None, 25, 1.0);
            draw_text(
                &prompt.title,
                960.0 - td.width * 0.5,
                modal_y + 38.0,
                25.0,
                Color::new(0.98, 0.92, 0.75, 1.0),
            );
            let sd = measure_text(&prompt.subtitle, None, 17, 1.0);
            draw_text(
                &prompt.subtitle,
                960.0 - sd.width * 0.5,
                modal_y + 66.0,
                17.0,
                Color::new(0.78, 0.74, 0.86, 1.0),
            );

            for (i, opt) in prompt.options.iter().enumerate() {
                let btn_rect = Rect::new(
                    modal_x + 28.0,
                    modal_y + 92.0 + (i as f32) * (opt_h + 10.0),
                    modal_w - 56.0,
                    opt_h,
                );
                let is_selected = opt
                    .target_uid
                    .map(|u| self.selected_prompt_uids.contains(&u))
                    .unwrap_or(false);
                let accent = if is_selected {
                    Color::new(0.32, 0.92, 0.58, 1.0)
                } else {
                    Color::new(0.38, 0.62, 0.92, 1.0)
                };
                draw_ui_button(
                    btn_rect,
                    &opt.label,
                    if i == 0 { "[Click or Space]" } else { "[Click to Select]" },
                    true,
                    accent,
                );
            }
        }

        // 5. Particles, Floating Combat Text, and Turn Banners
        self.vfx.draw_layer4_vfx();

        // 6. Immediate-mode Debug & Telemetry Overlay (toggle with F3 / Tab)
        if self.show_debug_overlay {
            let fps = get_fps();
            let ft_ms = get_frame_time() * 1000.0;
            let active_particles = self.vfx.pool.iter().filter(|p| p.active).count();
            let q_len = self.sequencer.queue.len();
            let panel = Rect::new(1687.0, 158.0, 185.0, 260.0);
            draw_rectangle(
                panel.x,
                panel.y,
                panel.w,
                panel.h,
                Color::new(0.06, 0.05, 0.09, 0.90),
            );
            draw_rectangle_lines(
                panel.x,
                panel.y,
                panel.w,
                panel.h,
                1.5,
                Color::new(0.32, 0.88, 0.62, 0.75),
            );
            draw_text(
                "TELEMETRY [Tab/F3]",
                panel.x + 10.0,
                panel.y + 22.0,
                15.0,
                Color::new(0.35, 0.95, 0.65, 1.0),
            );
            let lines = [
                format!("FPS: {fps} ({ft_ms:.2} ms)"),
                format!("Seed: {}", self.board_state.seed),
                format!("Event Queue: {q_len}"),
                format!(
                    "Blocking Anim: {}",
                    if self.sequencer.active_blocking.is_some() {
                        "YES"
                    } else {
                        "NO"
                    }
                ),
                format!("Particles: {}/{}", active_particles, self.vfx.pool.len()),
                format!("Camera Trauma: {:.2}", self.camera.trauma),
                format!("Hand Cards: {}", self.hand_visuals.len()),
                format!(
                    "Units: {} vs {}",
                    self.board_visuals[0].len(),
                    self.board_visuals[1].len()
                ),
                "Drag Card -> Table".to_string(),
                "Drag Unit -> Target".to_string(),
            ];
            for (i, line) in lines.iter().enumerate() {
                draw_text(
                    line,
                    panel.x + 10.0,
                    panel.y + 46.0 + (i as f32) * 21.0,
                    14.0,
                    Color::new(0.84, 0.90, 0.88, 1.0),
                );
            }
        }
    }
}

fn compute_board_slot_pos(index: usize, total: usize, y: f32) -> Vec2 {
    let count = total.max(1);
    let spacing = 152.0;
    let total_w = (count.saturating_sub(1) as f32) * spacing;
    let start_x = 960.0 - total_w * 0.5;
    vec2(start_x + (index as f32) * spacing, y)
}

#[allow(non_snake_case)]
fn draw_Rectangle_shield_aura(center: Vec2, size: Vec2, alpha: f32) {
    draw_rotated_rect(
        center,
        size,
        0.0,
        Color::new(0.35, 0.85, 1.0, alpha),
    );
    draw_rectangle_lines(
        center.x - size.x * 0.5,
        center.y - size.y * 0.5,
        size.x,
        size.y,
        2.5,
        Color::new(0.55, 0.95, 1.0, (alpha * 2.4).clamp(0.0, 1.0)),
    );
}

fn draw_ui_button(rect: Rect, title: &str, subtitle: &str, enabled: bool, accent: Color) {
    let bg = if enabled {
        Color::new(accent.r * 0.35, accent.g * 0.35, accent.b * 0.35, 0.95)
    } else {
        Color::new(0.14, 0.12, 0.16, 0.70)
    };
    let border = if enabled {
        accent
    } else {
        Color::new(0.38, 0.35, 0.42, 0.65)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.5, border);

    let td = measure_text(title, None, 19, 1.0);
    draw_text(
        title,
        rect.x + (rect.w - td.width) * 0.5,
        rect.y + 24.0,
        19.0,
        if enabled {
            WHITE
        } else {
            Color::new(0.6, 0.6, 0.65, 1.0)
        },
    );
    let sd = measure_text(subtitle, None, 13, 1.0);
    draw_text(
        subtitle,
        rect.x + (rect.w - sd.width) * 0.5,
        rect.y + rect.h - 9.0,
        13.0,
        Color::new(0.85, 0.82, 0.75, 0.90),
    );
}
