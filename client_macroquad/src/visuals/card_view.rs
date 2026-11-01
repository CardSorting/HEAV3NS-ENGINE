use crate::bridge::VisualCardData;
use keyframe::{ease, functions::{BezierCurve, EaseInOutCubic, EaseOutCubic}};
use macroquad::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub const CARD_WIDTH: f32 = 164.0;
pub const CARD_HEIGHT: f32 = 236.0;

/// Non-linear elastic easing curve for card scale magnification and bounce.
pub fn ease_out_elastic(a: f32, b: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0001 {
        return a;
    }
    if t >= 0.9999 {
        return b;
    }
    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
    let val = 2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0;
    a + (b - a) * val
}

/// Non-linear cubic-bezier overshoot easing using `keyframe::functions::BezierCurve`.
pub fn ease_out_back_bezier(a: f32, b: f32, t: f32) -> f32 {
    let curve = BezierCurve::from([0.34, 1.56].into(), [0.64, 1.0].into());
    ease(curve, a as f64, b as f64, t.clamp(0.0, 1.0) as f64) as f32
}

/// 2D/2.5D spatial transform for a card on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardTransform {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
    pub tilt_25d: Vec2,
    pub z_order: i32,
}

impl Default for CardTransform {
    fn default() -> Self {
        Self {
            position: vec2(960.0, 540.0),
            rotation: 0.0,
            scale: Vec2::ONE,
            tilt_25d: Vec2::ZERO,
            z_order: 0,
        }
    }
}

/// Every card on screen is represented by a `CardVisual` containing both its `target_transform`
/// and `current_transform`, smoothly damped every frame using `delta_time` (`get_frame_time()`)
/// and non-linear `keyframe` easing curves.
#[derive(Clone, Debug)]
pub struct CardVisual {
    pub data: VisualCardData,
    pub current_transform: CardTransform,
    pub target_transform: CardTransform,
    pub start_transform: CardTransform,
    pub velocity: Vec2,
    pub shadow_offset: Vec2,
    pub entrance_timer: f32,
    pub entrance_duration: f32,
    pub is_hovered: bool,
    pub is_dragging: bool,
    pub pulse_phase: f32,
}

impl CardVisual {
    pub fn new(data: VisualCardData, spawn_pos: Vec2, target: CardTransform) -> Self {
        let initial = CardTransform {
            position: spawn_pos,
            rotation: -0.35,
            scale: vec2(0.35, 0.35),
            tilt_25d: Vec2::ZERO,
            z_order: target.z_order,
        };
        Self {
            pulse_phase: (data.uid as f32) * 0.73,
            data,
            current_transform: initial,
            target_transform: target,
            start_transform: initial,
            velocity: Vec2::ZERO,
            shadow_offset: vec2(6.0, 10.0),
            entrance_timer: 0.0,
            entrance_duration: 0.45,
            is_hovered: false,
            is_dragging: false,
        }
    }

    /// Updates spring/tween interpolation and velocity-based tilt using `dt`.
    pub fn update(&mut self, dt: f32, cursor_vel: Vec2) {
        let dt_clamped = dt.clamp(0.0001, 0.05);
        self.pulse_phase += dt_clamped * 3.2;

        // Non-linear entrance tween using `keyframe` (BezierCurve & EaseOutCubic)
        if !self.is_dragging && self.entrance_timer < self.entrance_duration {
            self.entrance_timer = (self.entrance_timer + dt_clamped).min(self.entrance_duration);
            let t = (self.entrance_timer / self.entrance_duration).clamp(0.0, 1.0);
            let eased_scale = ease_out_back_bezier(0.35, 1.0, t);
            let eased_pos = ease(EaseOutCubic, 0.0, 1.0, t as f64) as f32;
            self.current_transform.position = self
                .start_transform
                .position
                .lerp(self.target_transform.position, eased_pos);
            self.current_transform.scale = self.target_transform.scale * eased_scale;
        } else if self.is_dragging {
            self.entrance_timer = self.entrance_duration;
        }

        // Exponential spring damping toward target_transform
        let pos_stiffness = if self.is_dragging { 28.0 } else if self.is_hovered { 20.0 } else { 14.0 };
        let scale_stiffness = if self.is_hovered { 22.0 } else { 15.0 };
        let rot_stiffness = 16.0;

        let alpha_pos = 1.0 - (-pos_stiffness * dt_clamped).exp();
        let alpha_scale = 1.0 - (-scale_stiffness * dt_clamped).exp();
        let alpha_rot = 1.0 - (-rot_stiffness * dt_clamped).exp();

        let prev_pos = self.current_transform.position;
        self.current_transform.position = self
            .current_transform
            .position
            .lerp(self.target_transform.position, alpha_pos);
        self.velocity = (self.current_transform.position - prev_pos) / dt_clamped;

        // Apply non-linear elastic scale overshoot when hovered
        let desired_scale = if self.is_hovered && !self.is_dragging {
            let osc = ease(
                EaseInOutCubic,
                0.85,
                1.0,
                (self.pulse_phase.sin() * 0.5 + 0.5) as f64,
            ) as f32;
            let hover_bounce = ease_out_elastic(1.0, 1.3, osc);
            vec2(hover_bounce.clamp(1.26, 1.33), hover_bounce.clamp(1.26, 1.33))
        } else {
            self.target_transform.scale
        };
        self.current_transform.scale = self
            .current_transform
            .scale
            .lerp(desired_scale, alpha_scale);

        // Velocity-Based Tilt: when dragging a card across the board, calculate cursor velocity
        // and apply proportional rotational tilt and drop-shadow offset to simulate weight & air resistance.
        let target_rot = if self.is_dragging {
            let v_x = (cursor_vel.x * 0.4 + self.velocity.x * 0.6).clamp(-1400.0, 1400.0);
            (v_x * 0.00028).clamp(-0.38, 0.38)
        } else {
            self.target_transform.rotation
        };

        self.current_transform.rotation =
            self.current_transform.rotation + (target_rot - self.current_transform.rotation) * alpha_rot;

        let target_tilt = if self.is_dragging {
            vec2(
                (self.velocity.x * 0.0005).clamp(-0.45, 0.45),
                (self.velocity.y * 0.0005).clamp(-0.45, 0.45),
            )
        } else if self.is_hovered {
            vec2(
                (self.pulse_phase * 1.4).sin() * 0.18,
                (self.pulse_phase * 1.1).cos() * 0.14,
            )
        } else {
            Vec2::ZERO
        };
        self.current_transform.tilt_25d = self
            .current_transform
            .tilt_25d
            .lerp(target_tilt, alpha_rot);

        let target_shadow = if self.is_dragging {
            vec2(
                18.0 - (self.velocity.x * 0.012).clamp(-14.0, 14.0),
                28.0 - (self.velocity.y * 0.012).clamp(-14.0, 14.0),
            )
        } else if self.is_hovered {
            vec2(12.0, 18.0)
        } else {
            vec2(5.0, 8.0)
        };
        self.shadow_offset = self.shadow_offset.lerp(target_shadow, alpha_pos);
        self.current_transform.z_order = self.target_transform.z_order;
    }

    pub fn bounding_rect(&self) -> Rect {
        let w = CARD_WIDTH * self.current_transform.scale.x;
        let h = CARD_HEIGHT * self.current_transform.scale.y;
        Rect::new(
            self.current_transform.position.x - w * 0.5,
            self.current_transform.position.y - h * 0.5,
            w,
            h,
        )
    }

    pub fn contains_point(&self, pt: Vec2) -> bool {
        // Inverse-rotate point around card center for accurate tilted hit-testing
        let delta = pt - self.current_transform.position;
        let cos_r = (-self.current_transform.rotation).cos();
        let sin_r = (-self.current_transform.rotation).sin();
        let local = vec2(
            delta.x * cos_r - delta.y * sin_r,
            delta.x * sin_r + delta.y * cos_r,
        );
        let half_w = (CARD_WIDTH * self.current_transform.scale.x) * 0.5;
        let half_h = (CARD_HEIGHT * self.current_transform.scale.y) * 0.5;
        local.x.abs() <= half_w && local.y.abs() <= half_h
    }
}

/// Calculates the dynamic quadratic Bezier / arc transform for a card at `index` out of `total`
/// in either the player's hand (`is_opponent = false`) or opponent's hand (`is_opponent = true`).
/// Hovered cards snap to the top of the hand layer, scale up (`1.3x`), and shift vertically so the
/// full card text is readable.
pub fn compute_hand_arc_transform(
    index: usize,
    total: usize,
    is_opponent: bool,
    is_hovered: bool,
) -> CardTransform {
    let count = total.max(1);
    let t = if count == 1 {
        0.5
    } else {
        index as f32 / (count - 1) as f32
    };
    let centered = (t - 0.5) * 2.0; // [-1.0, 1.0]

    let max_spread = ((count as f32 - 1.0) * 118.0).min(760.0);
    let p0 = vec2(960.0 - max_spread * 0.5, if is_opponent { 52.0 } else { 995.0 });
    let p1 = vec2(960.0, if is_opponent { 112.0 } else { 912.0 });
    let p2 = vec2(960.0 + max_spread * 0.5, if is_opponent { 52.0 } else { 995.0 });

    // Quadratic bezier evaluation B(t) = (1-t)^2 P0 + 2(1-t)t P1 + t^2 P2
    let one_minus_t = 1.0 - t;
    let mut pos = p0 * (one_minus_t * one_minus_t)
        + p1 * (2.0 * one_minus_t * t)
        + p2 * (t * t);

    let max_angle = (count as f32 * 0.032).min(0.24);
    let mut rotation = if is_opponent {
        -centered * max_angle * 0.75
    } else {
        centered * max_angle
    };

    let mut scale = if is_opponent {
        vec2(0.64, 0.64)
    } else {
        Vec2::ONE
    };
    let mut z_order = index as i32;

    if is_hovered && !is_opponent {
        pos.y = 858.0; // Shift vertically so full card text is readable inside 1920x1080
        rotation = 0.0;
        scale = vec2(1.3, 1.3);
        z_order = 100;
    }

    CardTransform {
        position: pos,
        rotation,
        scale,
        tilt_25d: Vec2::ZERO,
        z_order,
    }
}

/// Custom GLSL 2.5D Foil & Playable Border Glow Shaders compiled via `load_material`.
pub struct CardShaderMaterials {
    pub playable_glow_material: Option<Material>,
    pub foil_art_material: Option<Material>,
    pub card_textures: HashMap<String, Texture2D>,
    pub card_back_texture: Option<Texture2D>,
}

const DEFAULT_VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    color = color0 / 255.0;
    uv = texcoord;
}
"#;

/// Custom GLSL fragment shader for playable card borders (green/gold pulsing glow).
const PLAYABLE_GLOW_FRAGMENT_SHADER: &str = r#"#version 100
precision mediump float;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform sampler2D Texture;
uniform vec4 GlowParams; // x: time, y: intensity, z: gold_mix, w: unused

void main() {
    vec4 tex = texture2D(Texture, uv);
    vec2 d = abs(uv - vec2(0.5)) * 2.0;
    float edge = max(d.x, d.y);
    float border_mask = smoothstep(0.76, 0.99, edge);
    float pulse = 0.62 + 0.38 * sin(GlowParams.x * 4.5 + (uv.x + uv.y) * 6.2831);
    vec3 emerald = vec3(0.22, 0.96, 0.54);
    vec3 gold = vec3(1.0, 0.84, 0.32);
    vec3 glow_col = mix(emerald, gold, GlowParams.z * (0.5 + 0.5 * sin(GlowParams.x * 2.3 + uv.x * 4.0)));
    float alpha = border_mask * pulse * GlowParams.y * color.a;
    gl_FragColor = vec4(glow_col + tex.rgb * 0.15, alpha);
}
"#;

/// Custom GLSL fragment shader for 2.5D holographic foil card art tilts.
const FOIL_ART_FRAGMENT_SHADER: &str = r#"#version 100
precision mediump float;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform sampler2D Texture;
uniform vec4 FoilParams; // x: time, y: tilt_x, z: tilt_y, w: foil_strength

void main() {
    vec4 base = texture2D(Texture, uv) * color;
    float diag = (uv.x * 1.35 + uv.y * 0.95) + (FoilParams.y * 1.8 - FoilParams.z * 1.4) + FoilParams.x * 0.35;
    float sheen = pow(max(0.0, sin(diag * 7.5)), 4.0);
    vec3 rainbow = 0.5 + 0.5 * cos(6.28318 * (diag + vec3(0.0, 0.33, 0.67)));
    vec3 final_rgb = base.rgb + rainbow * sheen * FoilParams.w * 0.36;
    gl_FragColor = vec4(final_rgb, base.a);
}
"#;

impl CardShaderMaterials {
    pub fn load() -> Self {
        let playable_glow_material = load_material(
            ShaderSource::Glsl {
                vertex: DEFAULT_VERTEX_SHADER,
                fragment: PLAYABLE_GLOW_FRAGMENT_SHADER,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(miniquad::BlendState::new(
                        miniquad::Equation::Add,
                        miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
                        miniquad::BlendFactor::OneMinusValue(miniquad::BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                uniforms: vec![UniformDesc::new("GlowParams", UniformType::Float4)],
                ..Default::default()
            },
        )
        .ok();

        let foil_art_material = load_material(
            ShaderSource::Glsl {
                vertex: DEFAULT_VERTEX_SHADER,
                fragment: FOIL_ART_FRAGMENT_SHADER,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(miniquad::BlendState::new(
                        miniquad::Equation::Add,
                        miniquad::BlendFactor::Value(miniquad::BlendValue::SourceAlpha),
                        miniquad::BlendFactor::OneMinusValue(miniquad::BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                uniforms: vec![UniformDesc::new("FoilParams", UniformType::Float4)],
                ..Default::default()
            },
        )
        .ok();

        let mut card_textures = HashMap::new();
        let card_back_texture = Self::load_webp_texture("assets/cards/card-back.webp")
            .or_else(|| Self::load_webp_texture("../assets/cards/card-back.webp"));

        // Preload card art textures from `assets/cards/*.webp`
        for dir in ["assets/cards", "../assets/cards"] {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("webp") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if stem != "card-back" && !card_textures.contains_key(stem) {
                                if let Some(tex) = Self::load_webp_texture_from_path(&path) {
                                    card_textures.insert(stem.to_string(), tex);
                                }
                            }
                        }
                    }
                }
            }
        }

        Self {
            playable_glow_material,
            foil_art_material,
            card_textures,
            card_back_texture,
        }
    }

    fn load_webp_texture(rel_path: &str) -> Option<Texture2D> {
        Self::load_webp_texture_from_path(Path::new(rel_path))
    }

    fn load_webp_texture_from_path(path: &Path) -> Option<Texture2D> {
        let bytes = std::fs::read(path).ok()?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let width = img.width() as u16;
        let height = img.height() as u16;
        let raw = img.into_raw();
        let tex = Texture2D::from_rgba8(width, height, &raw);
        tex.set_filter(FilterMode::Linear);
        Some(tex)
    }
}

/// Renders a single `CardVisual` with drop shadow, 2.5D holographic foil shader, playable border glow
/// shader, cost crystal, combo badge, and crisp typography.
pub fn draw_card_visual(
    card: &CardVisual,
    materials: &CardShaderMaterials,
    time: f32,
    is_highlighted_target: bool,
) {
    let pos = card.current_transform.position;
    let scale = card.current_transform.scale;
    let rot = card.current_transform.rotation;
    let w = CARD_WIDTH * scale.x;
    let h = CARD_HEIGHT * scale.y;

    // 1. Dynamic velocity-linked drop shadow
    draw_rotated_rect(
        pos + card.shadow_offset,
        vec2(w, h),
        rot,
        Color::new(0.02, 0.01, 0.03, if card.is_dragging { 0.62 } else { 0.45 }),
    );

    // 2. Playable Green/Gold Pulsing Glow Shader behind card border
    if card.data.is_playable || is_highlighted_target {
        if let Some(ref mat) = materials.playable_glow_material {
            gl_use_material(mat);
            let gold_mix = if is_highlighted_target {
                0.95
            } else if card.data.combo_n.is_some() {
                0.65
            } else {
                0.28
            };
            let intensity = if card.is_hovered || card.is_dragging || is_highlighted_target {
                1.0
            } else {
                0.82
            };
            mat.set_uniform("GlowParams", vec4(time + card.pulse_phase, intensity, gold_mix, 0.0));
            draw_rotated_rect(
                pos,
                vec2(w + 18.0 * scale.x, h + 18.0 * scale.y),
                rot,
                WHITE,
            );
            gl_use_default_material();
        } else {
            let pulse = 0.55 + 0.45 * (time * 4.5 + card.pulse_phase).sin();
            draw_rotated_rect(
                pos,
                vec2(w + 10.0 * scale.x, h + 10.0 * scale.y),
                rot,
                Color::new(0.25, 0.95, 0.52, 0.65 * pulse),
            );
        }
    }

    // 3. Outer ornate card frame
    let frame_col = match card.data.card_type.as_str() {
        "Character" => Color::new(0.23, 0.16, 0.28, 1.0),
        "Action" => Color::new(0.15, 0.22, 0.31, 1.0),
        "Counter" => Color::new(0.32, 0.15, 0.19, 1.0),
        _ => Color::new(0.20, 0.18, 0.22, 1.0),
    };
    let trim_col = if card.data.owner == 0 {
        Color::new(0.84, 0.69, 0.39, 1.0) // Alice warm gold
    } else {
        Color::new(0.76, 0.36, 0.44, 1.0) // White Rabbit crimson rose
    };

    draw_rotated_rect(pos, vec2(w, h), rot, trim_col);
    draw_rotated_rect(pos, vec2(w - 6.0 * scale.x, h - 6.0 * scale.y), rot, frame_col);

    // 4. Card Art with 2.5D Holographic Foil Shader
    let art_w = w * 0.86;
    let art_h = h * 0.46;
    let up_dir = vec2(rot.sin(), -rot.cos());
    let right_dir = vec2(rot.cos(), rot.sin());
    let art_center = pos + up_dir * (h * 0.14);

    if let Some(tex) = materials.card_textures.get(&card.data.spec_id) {
        if let Some(ref foil_mat) = materials.foil_art_material {
            gl_use_material(foil_mat);
            let foil_strength = if card.is_hovered || card.is_dragging {
                0.95
            } else if card.data.combo_n.is_some() {
                0.55
            } else {
                0.30
            };
            foil_mat.set_uniform(
                "FoilParams",
                vec4(
                    time,
                    card.current_transform.tilt_25d.x,
                    card.current_transform.tilt_25d.y,
                    foil_strength,
                ),
            );
        }
        draw_texture_ex(
            tex,
            art_center.x - art_w * 0.5,
            art_center.y - art_h * 0.5,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(art_w, art_h)),
                rotation: rot,
                pivot: Some(art_center),
                ..Default::default()
            },
        );
        gl_use_default_material();
    } else {
        draw_rotated_rect(
            art_center,
            vec2(art_w, art_h),
            rot,
            Color::new(0.32, 0.24, 0.42, 1.0),
        );
    }

    // 5. Title Banner & Text Box
    let title_pos = pos - up_dir * (h * 0.14);
    draw_rotated_rect(
        title_pos,
        vec2(w * 0.88, 22.0 * scale.y),
        rot,
        Color::new(0.11, 0.08, 0.14, 0.95),
    );

    let text_box_pos = pos - up_dir * (h * 0.31);
    draw_rotated_rect(
        text_box_pos,
        vec2(w * 0.88, h * 0.26),
        rot,
        Color::new(0.94, 0.90, 0.82, 0.96),
    );

    // 6. Typography (Title, Type, Rules Text)
    let title_font_size = (15.0 * scale.y).clamp(11.0, 24.0) as u16;
    let short_title = truncate_str(&card.data.title, 20);
    let title_dims = measure_text(&short_title, None, title_font_size, 1.0);
    draw_text(
        &short_title,
        title_pos.x - title_dims.width * 0.5,
        title_pos.y + title_dims.height * 0.32,
        title_font_size as f32,
        Color::new(0.98, 0.93, 0.78, 1.0),
    );

    // Type & Combo subheader
    let type_label = if let Some(cn) = card.data.combo_n {
        format!("{} - Combo {}", card.data.card_type, cn)
    } else {
        card.data.card_type.clone()
    };
    let sub_font = (12.0 * scale.y).clamp(9.0, 18.0) as u16;
    let sub_pos = pos - up_dir * (h * 0.05);
    let sub_dims = measure_text(&type_label, None, sub_font, 1.0);
    draw_text(
        &type_label,
        sub_pos.x - sub_dims.width * 0.5,
        sub_pos.y + sub_dims.height * 0.3,
        sub_font as f32,
        Color::new(0.86, 0.76, 0.54, 1.0),
    );

    // Wrapped card rules text inside parchment box
    let body_font = (11.5 * scale.y).clamp(9.0, 17.0) as u16;
    let max_chars = if card.is_hovered { 28 } else { 24 };
    let wrapped = wrap_card_text(&card.data.text, max_chars, if card.is_hovered { 5 } else { 4 });
    let start_y = text_box_pos.y - (h * 0.09);
    for (line_idx, line) in wrapped.iter().enumerate() {
        draw_text(
            line,
            text_box_pos.x - w * 0.41,
            start_y + (line_idx as f32) * (body_font as f32 * 1.15),
            body_font as f32,
            Color::new(0.14, 0.10, 0.16, 1.0),
        );
    }

    // 7. Top-Left Mana / Action Cost Gem
    let cost_center = pos - right_dir * (w * 0.41) + up_dir * (h * 0.42);
    let gem_r = 16.0 * scale.x;
    draw_circle(cost_center.x, cost_center.y, gem_r + 2.0, trim_col);
    draw_circle(
        cost_center.x,
        cost_center.y,
        gem_r,
        Color::new(0.14, 0.48, 0.86, 1.0),
    );
    let cost_str = card.data.cost.to_string();
    let cost_font = (19.0 * scale.y).clamp(12.0, 28.0) as u16;
    let cd = measure_text(&cost_str, None, cost_font, 1.0);
    draw_text(
        &cost_str,
        cost_center.x - cd.width * 0.5,
        cost_center.y + cd.height * 0.35,
        cost_font as f32,
        WHITE,
    );

    // 8. Bottom Attack & Health / Shield Badges for Characters
    if card.data.card_type == "Character" {
        let atk_center = pos - right_dir * (w * 0.39) - up_dir * (h * 0.43);
        let hp_center = pos + right_dir * (w * 0.39) - up_dir * (h * 0.43);
        let badge_r = 14.0 * scale.x;

        draw_circle(atk_center.x, atk_center.y, badge_r + 1.5, trim_col);
        draw_circle(
            atk_center.x,
            atk_center.y,
            badge_r,
            Color::new(0.78, 0.24, 0.20, 1.0),
        );
        let atk_s = card.data.attack_badge.to_string();
        let ad = measure_text(&atk_s, None, (16.0 * scale.y) as u16, 1.0);
        draw_text(
            &atk_s,
            atk_center.x - ad.width * 0.5,
            atk_center.y + ad.height * 0.35,
            16.0 * scale.y,
            WHITE,
        );

        draw_circle(hp_center.x, hp_center.y, badge_r + 1.5, trim_col);
        draw_circle(
            hp_center.x,
            hp_center.y,
            badge_r,
            Color::new(0.20, 0.64, 0.34, 1.0),
        );
        let hp_s = card.data.health_badge.to_string();
        let hd = measure_text(&hp_s, None, (16.0 * scale.y) as u16, 1.0);
        draw_text(
            &hp_s,
            hp_center.x - hd.width * 0.5,
            hp_center.y + hd.height * 0.35,
            16.0 * scale.y,
            WHITE,
        );
    }
}

/// Renders an opponent face-down card back along the upper hand arc or shield row.
pub fn draw_card_back(
    transform: &CardTransform,
    materials: &CardShaderMaterials,
    tint: Color,
) {
    let pos = transform.position;
    let w = CARD_WIDTH * transform.scale.x;
    let h = CARD_HEIGHT * transform.scale.y;
    let rot = transform.rotation;

    draw_rotated_rect(
        pos + vec2(4.0, 6.0),
        vec2(w, h),
        rot,
        Color::new(0.02, 0.01, 0.03, 0.45),
    );
    draw_rotated_rect(
        pos,
        vec2(w + 4.0, h + 4.0),
        rot,
        Color::new(0.78, 0.62, 0.34, 1.0),
    );

    if let Some(ref tex) = materials.card_back_texture {
        draw_texture_ex(
            tex,
            pos.x - w * 0.5,
            pos.y - h * 0.5,
            tint,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                rotation: rot,
                pivot: Some(pos),
                ..Default::default()
            },
        );
    } else {
        draw_rotated_rect(pos, vec2(w, h), rot, Color::new(0.26, 0.13, 0.21, 1.0));
        draw_rotated_rect(
            pos,
            vec2(w * 0.78, h * 0.78),
            rot,
            Color::new(0.45, 0.24, 0.34, 1.0),
        );
    }
}

pub fn draw_rotated_rect(center: Vec2, size: Vec2, rotation: f32, color: Color) {
    draw_rectangle_ex(
        center.x,
        center.y,
        size.x,
        size.y,
        DrawRectangleParams {
            offset: vec2(0.5, 0.5),
            rotation,
            color,
        },
    );
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

fn wrap_card_text(text: &str, max_width_chars: usize, max_lines: usize) -> Vec<String> {
    let cleaned = text.replace('\n', " ");
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in cleaned.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= max_width_chars {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            if lines.len() >= max_lines {
                return lines;
            }
            current = word.to_string();
        }
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }
    lines
}
