use crate::visuals::card_view::ease_out_back_bezier;
use keyframe::{ease, functions::EaseOutCubic};
use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

pub const VIRTUAL_WIDTH: f32 = 1920.0;
pub const VIRTUAL_HEIGHT: f32 = 1080.0;

/// Trauma-based Camera Controller wrapping the board rendering in a custom `Camera2D`.
/// Exposes `trigger_shake(intensity: f32, duration: f32)` using continuous Perlin/simplex-like
/// gradient noise decay for heavy minion impacts and shield shatters.
#[derive(Clone, Debug)]
pub struct CameraController {
    pub trauma: f32,
    pub max_offset: Vec2,
    pub max_rotation_deg: f32,
    pub decay_rate: f32,
    pub noise_time: f32,
    pub current_offset: Vec2,
    pub current_rotation_deg: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            trauma: 0.0,
            max_offset: vec2(32.0, 24.0),
            max_rotation_deg: 2.2,
            decay_rate: 1.8,
            noise_time: 0.0,
            current_offset: Vec2::ZERO,
            current_rotation_deg: 0.0,
        }
    }

    /// Triggers trauma-based screen shake with `intensity` (0.0..=1.5) and `duration` in seconds.
    pub fn trigger_shake(&mut self, intensity: f32, duration: f32) {
        let clamped_dur = duration.max(0.05);
        self.trauma = (self.trauma + intensity).clamp(0.0, 1.4);
        self.decay_rate = (self.trauma / clamped_dur).max(0.8);
    }

    pub fn update(&mut self, dt: f32) {
        self.noise_time += dt * 28.0;
        if self.trauma > 0.0 {
            self.trauma = (self.trauma - self.decay_rate * dt).max(0.0);
            let shake = self.trauma * self.trauma; // Quadratic trauma falloff
            let nx = simplex_1d(self.noise_time * 1.13 + 17.3);
            let ny = simplex_1d(self.noise_time * 1.37 + 53.9);
            let nr = simplex_1d(self.noise_time * 0.91 + 101.7);
            self.current_offset = vec2(self.max_offset.x * shake * nx, self.max_offset.y * shake * ny);
            self.current_rotation_deg = self.max_rotation_deg * shake * nr;
        } else {
            self.current_offset = Vec2::ZERO;
            self.current_rotation_deg = 0.0;
        }
    }

    /// Builds an aspect-ratio preserving `Camera2D` for the 1920x1080 virtual stage,
    /// including letterbox/pillarbox viewport scaling and trauma-based camera recoil.
    pub fn to_camera_2d(&self) -> Camera2D {
        let sw = screen_width().max(1.0);
        let sh = screen_height().max(1.0);
        let scale = (sw / VIRTUAL_WIDTH).min(sh / VIRTUAL_HEIGHT);
        let vp_w = (VIRTUAL_WIDTH * scale) as i32;
        let vp_h = (VIRTUAL_HEIGHT * scale) as i32;
        let vp_x = ((sw - VIRTUAL_WIDTH * scale) * 0.5) as i32;
        let vp_y = ((sh - VIRTUAL_HEIGHT * scale) * 0.5) as i32;

        Camera2D {
            target: vec2(
                VIRTUAL_WIDTH * 0.5 + self.current_offset.x,
                VIRTUAL_HEIGHT * 0.5 + self.current_offset.y,
            ),
            zoom: vec2(2.0 / VIRTUAL_WIDTH, 2.0 / VIRTUAL_HEIGHT),
            rotation: self.current_rotation_deg,
            viewport: Some((vp_x, vp_y, vp_w, vp_h)),
            ..Default::default()
        }
    }

    /// Converts physical window mouse coordinates to 1920x1080 virtual stage coordinates.
    pub fn screen_to_virtual(&self, raw_mouse: Vec2) -> Vec2 {
        let sw = screen_width().max(1.0);
        let sh = screen_height().max(1.0);
        let scale = (sw / VIRTUAL_WIDTH).min(sh / VIRTUAL_HEIGHT);
        let vp_x = (sw - VIRTUAL_WIDTH * scale) * 0.5;
        let vp_y = (sh - VIRTUAL_HEIGHT * scale) * 0.5;
        vec2(
            ((raw_mouse.x - vp_x) / scale).clamp(0.0, VIRTUAL_WIDTH),
            ((raw_mouse.y - vp_y) / scale).clamp(0.0, VIRTUAL_HEIGHT),
        )
    }
}

/// 1D continuous gradient / Perlin-simplex noise in [-1.0, 1.0].
fn simplex_1d(x: f32) -> f32 {
    let i0 = x.floor() as i32;
    let i1 = i0 + 1;
    let f0 = x - i0 as f32;
    let f1 = f0 - 1.0;

    let g0 = hash_grad(i0) * f0;
    let g1 = hash_grad(i1) * f1;

    // Quintic fade curve 6t^5 - 15t^4 + 10t^3
    let u = f0 * f0 * f0 * (f0 * (f0 * 6.0 - 15.0) + 10.0);
    (g0 + u * (g1 - g0)).clamp(-1.0, 1.0)
}

fn hash_grad(mut n: i32) -> f32 {
    n = (n << 13) ^ n;
    let m = n
        .wrapping_mul(n.wrapping_mul(n).wrapping_mul(15731).wrapping_add(789221))
        .wrapping_add(1376312589);
    1.0 - ((m & 0x7fffffff) as f32 / 1073741824.0)
}

/// Particle Emitter Preset loaded via `serde_json` or built-in defaults.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticlePreset {
    pub name: String,
    pub count: usize,
    pub min_speed: f32,
    pub max_speed: f32,
    pub min_life: f32,
    pub max_life: f32,
    pub start_size: f32,
    pub end_size: f32,
    pub gravity_y: f32,
    pub color_rgba: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub enum ParticleShape {
    Circle,
    Diamond,
    Shard,
    Spark,
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub active: bool,
    pub pos: Vec2,
    pub vel: Vec2,
    pub acc: Vec2,
    pub rotation: f32,
    pub angular_vel: f32,
    pub life: f32,
    pub max_life: f32,
    pub start_size: f32,
    pub end_size: f32,
    pub color: Color,
    pub shape: ParticleShape,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            active: false,
            pos: Vec2::ZERO,
            vel: Vec2::ZERO,
            acc: Vec2::ZERO,
            rotation: 0.0,
            angular_vel: 0.0,
            life: 0.0,
            max_life: 1.0,
            start_size: 8.0,
            end_size: 1.0,
            color: WHITE,
            shape: ParticleShape::Circle,
        }
    }
}

/// Active spell projectile flying across Layer 3 (Active Dragging & Projectiles).
#[derive(Clone, Debug)]
pub struct SpellProjectile {
    pub start: Vec2,
    pub control: Vec2,
    pub end: Vec2,
    pub elapsed: f32,
    pub duration: f32,
    pub color: Color,
    pub label: String,
}

impl SpellProjectile {
    pub fn current_pos(&self) -> Vec2 {
        let t = (self.elapsed / self.duration.max(0.01)).clamp(0.0, 1.0) as f64;
        let u = ease(EaseOutCubic, 0.0, 1.0, t) as f32;
        let om = 1.0 - u;
        self.start * (om * om) + self.control * (2.0 * om * u) + self.end * (u * u)
    }
}

/// Floating combat text & turn-change banner on Layer 4 (VFX & Screen Space UI).
#[derive(Clone, Debug)]
pub struct FloatingCombatText {
    pub text: String,
    pub pos: Vec2,
    pub vel: Vec2,
    pub elapsed: f32,
    pub duration: f32,
    pub color: Color,
    pub font_size: f32,
}

#[derive(Clone, Debug)]
pub struct TurnBanner {
    pub title: String,
    pub subtitle: String,
    pub elapsed: f32,
    pub duration: f32,
    pub accent: Color,
}

/// Lightweight pooled Sprite / Particle Emitter & Screen-Space VFX Manager (`src/visuals/vfx.rs`).
pub struct VfxManager {
    pub pool: Vec<Particle>,
    pub projectiles: Vec<SpellProjectile>,
    pub floating_texts: Vec<FloatingCombatText>,
    pub active_banner: Option<TurnBanner>,
    pub rng_counter: u64,
}

impl Default for VfxManager {
    fn default() -> Self {
        Self::new(512)
    }
}

impl VfxManager {
    pub fn new(pool_capacity: usize) -> Self {
        Self {
            pool: vec![Particle::default(); pool_capacity],
            projectiles: Vec::new(),
            floating_texts: Vec::new(),
            active_banner: None,
            rng_counter: 1337,
        }
    }

    fn next_rand_01(&mut self) -> f32 {
        self.rng_counter = self
            .rng_counter
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        ((self.rng_counter >> 32) as u32 as f32) / (u32::MAX as f32)
    }

    fn alloc_particle(&mut self) -> &mut Particle {
        if let Some(idx) = self.pool.iter().position(|p| !p.active) {
            return &mut self.pool[idx];
        }
        let idx = (self.rng_counter as usize) % self.pool.len();
        &mut self.pool[idx]
    }

    /// Spawns a ring of warm dust clouds when a minion hits the board.
    pub fn spawn_minion_dust_cloud(&mut self, center: Vec2) {
        for i in 0..34 {
            let angle = (i as f32 / 34.0) * std::f32::consts::TAU + self.next_rand_01() * 0.25;
            let speed = 75.0 + self.next_rand_01() * 195.0;
            let life = 0.45 + self.next_rand_01() * 0.40;
            let start_size = 14.0 + self.next_rand_01() * 16.0;
            let rot = self.next_rand_01() * std::f32::consts::TAU;
            let ang_v = (self.next_rand_01() - 0.5) * 4.0;
            let p = self.alloc_particle();
            *p = Particle {
                active: true,
                pos: center + vec2(angle.cos() * 24.0, angle.sin() * 12.0),
                vel: vec2(angle.cos() * speed, angle.sin() * speed * 0.55 - 25.0),
                acc: vec2(0.0, -40.0),
                rotation: rot,
                angular_vel: ang_v,
                life: 0.0,
                max_life: life,
                start_size,
                end_size: 2.0,
                color: Color::new(0.84, 0.76, 0.62, 0.78),
                shape: ParticleShape::Circle,
            };
        }
    }

    /// Spawns crystalline shards when mana crystals are spent or a Shield is shattered.
    pub fn spawn_mana_crystal_shatter(&mut self, center: Vec2, color: Color) {
        for _ in 0..28 {
            let angle = self.next_rand_01() * std::f32::consts::TAU;
            let speed = 90.0 + self.next_rand_01() * 260.0;
            let life = 0.38 + self.next_rand_01() * 0.35;
            let start_size = 9.0 + self.next_rand_01() * 10.0;
            let rot = self.next_rand_01() * std::f32::consts::TAU;
            let ang_v = (self.next_rand_01() - 0.5) * 12.0;
            let p = self.alloc_particle();
            *p = Particle {
                active: true,
                pos: center,
                vel: vec2(angle.cos() * speed, angle.sin() * speed - 60.0),
                acc: vec2(0.0, 320.0),
                rotation: rot,
                angular_vel: ang_v,
                life: 0.0,
                max_life: life,
                start_size,
                end_size: 1.5,
                color,
                shape: ParticleShape::Shard,
            };
        }
    }

    /// Spawns a high-energy spell impact burst.
    pub fn spawn_spell_impact(&mut self, center: Vec2, color: Color) {
        for _ in 0..36 {
            let angle = self.next_rand_01() * std::f32::consts::TAU;
            let speed = 110.0 + self.next_rand_01() * 310.0;
            let life = 0.30 + self.next_rand_01() * 0.35;
            let start_size = 8.0 + self.next_rand_01() * 9.0;
            let rot = angle;
            let p = self.alloc_particle();
            *p = Particle {
                active: true,
                pos: center,
                vel: vec2(angle.cos() * speed, angle.sin() * speed),
                acc: Vec2::ZERO,
                rotation: rot,
                angular_vel: 4.0,
                life: 0.0,
                max_life: life,
                start_size,
                end_size: 0.5,
                color,
                shape: ParticleShape::Diamond,
            };
        }
    }

    pub fn spawn_projectile(&mut self, start: Vec2, end: Vec2, color: Color, label: &str) {
        let mid = (start + end) * 0.5 + vec2(0.0, -140.0);
        self.projectiles.push(SpellProjectile {
            start,
            control: mid,
            end,
            elapsed: 0.0,
            duration: 0.42,
            color,
            label: label.to_string(),
        });
    }

    pub fn spawn_floating_text(&mut self, text: impl Into<String>, pos: Vec2, color: Color, font_size: f32) {
        self.floating_texts.push(FloatingCombatText {
            text: text.into(),
            pos,
            vel: vec2(0.0, -68.0),
            elapsed: 0.0,
            duration: 1.15,
            color,
            font_size,
        });
    }

    pub fn show_banner(&mut self, title: impl Into<String>, subtitle: impl Into<String>, accent: Color) {
        self.active_banner = Some(TurnBanner {
            title: title.into(),
            subtitle: subtitle.into(),
            elapsed: 0.0,
            duration: 1.35,
            accent,
        });
    }

    pub fn update(&mut self, dt: f32) {
        // Update pooled particles
        for p in &mut self.pool {
            if !p.active {
                continue;
            }
            p.life += dt;
            if p.life >= p.max_life {
                p.active = false;
                continue;
            }
            p.vel += p.acc * dt;
            p.pos += p.vel * dt;
            p.rotation += p.angular_vel * dt;
        }

        // Update spell projectiles & emit trail sparks
        let mut finished_proj = Vec::new();
        let mut trail_spawns = Vec::new();
        for (idx, proj) in self.projectiles.iter_mut().enumerate() {
            proj.elapsed += dt;
            let pos = proj.current_pos();
            if proj.elapsed >= proj.duration {
                finished_proj.push((idx, proj.end, proj.color));
            } else {
                trail_spawns.push((pos, proj.color));
            }
        }
        for (pos, col) in trail_spawns {
            let p = self.alloc_particle();
            *p = Particle {
                active: true,
                pos,
                vel: vec2(0.0, 20.0),
                acc: Vec2::ZERO,
                rotation: 0.0,
                angular_vel: 3.0,
                life: 0.0,
                max_life: 0.22,
                start_size: 10.0,
                end_size: 1.0,
                color: col,
                shape: ParticleShape::Diamond,
            };
        }
        for (idx, end_pos, col) in finished_proj.into_iter().rev() {
            self.projectiles.remove(idx);
            self.spawn_spell_impact(end_pos, col);
        }

        // Update floating combat text
        self.floating_texts.retain_mut(|ft| {
            ft.elapsed += dt;
            ft.pos += ft.vel * dt;
            ft.elapsed < ft.duration
        });

        // Update turn banner
        if let Some(ref mut b) = self.active_banner {
            b.elapsed += dt;
            if b.elapsed >= b.duration {
                self.active_banner = None;
            }
        }
    }

    /// Renders active spell projectiles on Layer 3.
    pub fn draw_layer3_projectiles(&self) {
        for proj in &self.projectiles {
            let pos = proj.current_pos();
            draw_circle(pos.x, pos.y, 18.0, Color::new(proj.color.r, proj.color.g, proj.color.b, 0.38));
            draw_circle(pos.x, pos.y, 10.0, proj.color);
            draw_circle(pos.x, pos.y, 5.0, WHITE);
        }
    }

    /// Renders particles, floating combat text, and turn-change banners on Layer 4.
    pub fn draw_layer4_vfx(&self) {
        for p in &self.pool {
            if !p.active {
                continue;
            }
            let t = (p.life / p.max_life).clamp(0.0, 1.0);
            let size = p.start_size + (p.end_size - p.start_size) * t;
            let alpha = (1.0 - t) * p.color.a;
            let col = Color::new(p.color.r, p.color.g, p.color.b, alpha);

            match p.shape {
                ParticleShape::Circle => {
                    draw_circle(p.pos.x, p.pos.y, size * 0.5, col);
                }
                ParticleShape::Diamond | ParticleShape::Shard | ParticleShape::Spark => {
                    draw_rectangle_ex(
                        p.pos.x,
                        p.pos.y,
                        size,
                        size * 0.65,
                        DrawRectangleParams {
                            offset: vec2(0.5, 0.5),
                            rotation: p.rotation,
                            color: col,
                        },
                    );
                }
            }
        }

        for ft in &self.floating_texts {
            let t = (ft.elapsed / ft.duration).clamp(0.0, 1.0);
            let alpha = (1.0 - t * t).clamp(0.0, 1.0);
            let scale_pop = ease_out_back_bezier(0.6, 1.0, (t * 3.0).min(1.0));
            let fsize = (ft.font_size * scale_pop).clamp(14.0, 48.0) as u16;
            let dims = measure_text(&ft.text, None, fsize, 1.0);
            // Shadow
            draw_text(
                &ft.text,
                ft.pos.x - dims.width * 0.5 + 2.0,
                ft.pos.y + 2.0,
                fsize as f32,
                Color::new(0.0, 0.0, 0.0, alpha * 0.85),
            );
            draw_text(
                &ft.text,
                ft.pos.x - dims.width * 0.5,
                ft.pos.y,
                fsize as f32,
                Color::new(ft.color.r, ft.color.g, ft.color.b, alpha),
            );
        }

        if let Some(ref b) = self.active_banner {
            let t = (b.elapsed / b.duration).clamp(0.0, 1.0);
            let env = if t < 0.2 {
                ease_out_back_bezier(0.0, 1.0, t / 0.2)
            } else if t > 0.75 {
                1.0 - ((t - 0.75) / 0.25)
            } else {
                1.0
            };
            let alpha = env.clamp(0.0, 1.0);
            let banner_h = 84.0 * env.max(0.2);
            let cy = 500.0;
            draw_rectangle(
                360.0,
                cy - banner_h * 0.5,
                1200.0,
                banner_h,
                Color::new(0.09, 0.05, 0.12, 0.88 * alpha),
            );
            draw_rectangle_lines(
                360.0,
                cy - banner_h * 0.5,
                1200.0,
                banner_h,
                3.0,
                Color::new(b.accent.r, b.accent.g, b.accent.b, alpha),
            );
            let td = measure_text(&b.title, None, 34, 1.0);
            draw_text(
                &b.title,
                960.0 - td.width * 0.5,
                cy - 4.0,
                34.0,
                Color::new(0.98, 0.93, 0.78, alpha),
            );
            let sd = measure_text(&b.subtitle, None, 19, 1.0);
            draw_text(
                &b.subtitle,
                960.0 - sd.width * 0.5,
                cy + 24.0,
                19.0,
                Color::new(b.accent.r, b.accent.g, b.accent.b, alpha),
            );
        }
    }
}

/// Synthesizes procedural WAV audio buffers in memory and plays them via `macroquad::audio`
/// so the client has crisp tactile SFX and an ambient tavern/table loop with zero external audio files needed.
pub struct ProceduralAudioBank {
    pub sfx_card_hover: Option<Sound>,
    pub sfx_card_play: Option<Sound>,
    pub sfx_minion_impact: Option<Sound>,
    pub sfx_shield_shatter: Option<Sound>,
    pub sfx_turn_chime: Option<Sound>,
    pub ambient_loop: Option<Sound>,
}

impl ProceduralAudioBank {
    pub async fn init() -> Self {
        let sfx_card_hover = load_sound_from_bytes(&synth_wav(0.06, 520.0, 680.0, 0.18)).await.ok();
        let sfx_card_play = load_sound_from_bytes(&synth_wav(0.16, 260.0, 440.0, 0.35)).await.ok();
        let sfx_minion_impact = load_sound_from_bytes(&synth_wav(0.24, 140.0, 58.0, 0.50)).await.ok();
        let sfx_shield_shatter = load_sound_from_bytes(&synth_wav(0.30, 880.0, 220.0, 0.45)).await.ok();
        let sfx_turn_chime = load_sound_from_bytes(&synth_wav(0.32, 440.0, 660.0, 0.32)).await.ok();
        let ambient_loop = load_sound_from_bytes(&synth_wav(2.0, 110.0, 112.0, 0.04)).await.ok();

        if let Some(ref amb) = ambient_loop {
            play_sound(
                amb,
                PlaySoundParams {
                    looped: true,
                    volume: 0.05,
                },
            );
        }

        Self {
            sfx_card_hover,
            sfx_card_play,
            sfx_minion_impact,
            sfx_shield_shatter,
            sfx_turn_chime,
            ambient_loop,
        }
    }

    pub fn play_hover(&self) {
        if let Some(ref s) = self.sfx_card_hover {
            play_sound(s, PlaySoundParams { looped: false, volume: 0.16 });
        }
    }

    pub fn play_card(&self) {
        if let Some(ref s) = self.sfx_card_play {
            play_sound(s, PlaySoundParams { looped: false, volume: 0.32 });
        }
    }

    pub fn play_impact(&self) {
        if let Some(ref s) = self.sfx_minion_impact {
            play_sound(s, PlaySoundParams { looped: false, volume: 0.44 });
        }
    }

    pub fn play_shatter(&self) {
        if let Some(ref s) = self.sfx_shield_shatter {
            play_sound(s, PlaySoundParams { looped: false, volume: 0.40 });
        }
    }

    pub fn play_chime(&self) {
        if let Some(ref s) = self.sfx_turn_chime {
            play_sound(s, PlaySoundParams { looped: false, volume: 0.30 });
        }
    }
}

/// Generates a valid 16-bit mono PCM WAV file in memory with a frequency sweep and exponential decay.
fn synth_wav(duration_sec: f32, start_hz: f32, end_hz: f32, amplitude: f32) -> Vec<u8> {
    let sample_rate: u32 = 22050;
    let num_samples = (duration_sec * sample_rate as f32) as usize;
    let data_bytes = (num_samples * 2) as u32;
    let total_size = 36 + data_bytes;

    let mut wav = Vec::with_capacity(44 + num_samples * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&total_size.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
    wav.extend_from_slice(&1u16.to_le_bytes()); // NumChannels = 1
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // ByteRate
    wav.extend_from_slice(&2u16.to_le_bytes()); // BlockAlign
    wav.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());

    let mut phase = 0.0f32;
    for i in 0..num_samples {
        let t = i as f32 / num_samples.max(1) as f32;
        let freq = start_hz + (end_hz - start_hz) * t;
        phase += std::f32::consts::TAU * freq / sample_rate as f32;
        let env = (1.0 - t).powf(1.6) * (t * 25.0).min(1.0);
        let sample = ((phase.sin() * 0.75 + (phase * 2.0).sin() * 0.25) * amplitude * env * 32767.0)
            .clamp(-32767.0, 32767.0) as i16;
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}
