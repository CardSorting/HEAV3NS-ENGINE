use client_macroquad::renderer::TableStageRenderer;
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "WONDERLAND - The Enchanted Table (Macroquad 0.4 Client)".to_owned(),
        window_width: 1920,
        window_height: 1080,
        high_dpi: true,
        window_resizable: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut seed: u64 = 42;
    let mut starter: usize = 0;
    let mut max_frames: Option<u64> = None;
    let mut verify_gpu = false;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" if i + 1 < args.len() => {
                if let Ok(s) = args[i + 1].parse::<u64>() {
                    seed = s;
                }
                i += 2;
            }
            "--starter" if i + 1 < args.len() => {
                if let Ok(st) = args[i + 1].parse::<usize>() {
                    starter = st.min(1);
                }
                i += 2;
            }
            "--frames" if i + 1 < args.len() => {
                if let Ok(f) = args[i + 1].parse::<u64>() {
                    max_frames = Some(f);
                }
                i += 2;
            }
            "--verify-gpu" => {
                verify_gpu = true;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    let mut stage = TableStageRenderer::new(seed, starter)
        .await
        .expect("Failed to initialize Macroquad Enchanted Table stage");

    if verify_gpu {
        assert!(
            stage.materials.playable_glow_material.is_some(),
            "Playable glow GLSL shader material failed to compile"
        );
        assert!(
            stage.materials.foil_art_material.is_some(),
            "2.5D holographic foil GLSL shader material failed to compile"
        );
        assert_eq!(
            stage.materials.card_textures.len(),
            44,
            "Expected all 44 card WebP textures to load into GPU memory"
        );
        assert!(
            stage.materials.card_back_texture.is_some(),
            "Expected card-back.webp texture to load into GPU memory"
        );
    }

    let mut frame_count: u64 = 0;
    loop {
        let dt = get_frame_time().clamp(0.0001, 0.05);
        stage.update(dt);
        stage.render();

        frame_count += 1;
        if let Some(limit) = max_frames {
            if frame_count >= limit {
                break;
            }
        }
        next_frame().await;
    }
}

