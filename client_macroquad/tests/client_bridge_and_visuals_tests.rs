use client_macroquad::{
    bridge::{AnimationSequencer, GameBridge, GameCommand, VisualBoardState, VisualEvent},
    visuals::{
        card_view::{
            compute_hand_arc_transform, ease_out_back_bezier, ease_out_elastic, CardTransform,
            CardVisual,
        },
        vfx::{CameraController, VfxManager},
    },
};
use macroquad::prelude::vec2;

#[test]
fn test_bridge_initialization_and_5_card_starting_hand() {
    let (bridge, mut initial_events, initial_state) =
        GameBridge::new(42, 0).expect("GameBridge must initialize cleanly");

    // Turn 1 draw gives Alice 6 cards in hand (5 opening + 1 turn draw), and emits TurnStarted + CardDrawn
    assert_eq!(bridge.seed, 42);
    assert_eq!(initial_state.active_seat, 0);
    assert_eq!(initial_state.mana[0], 6);
    assert_eq!(initial_state.shields_count, [6, 6]);
    assert!(initial_state.player_hand.len() >= 5);
    assert!(!initial_events.is_empty());

    let first_ev = initial_events.pop_front().unwrap();
    assert!(matches!(
        first_ev,
        VisualEvent::TurnStarted {
            seat: 0,
            turn_number: 1
        }
    ));
}

#[test]
fn test_play_card_command_emits_mana_spent_and_unit_or_spell_events() {
    let (mut bridge, _, state) = GameBridge::new(42, 0).unwrap();

    // Find first playable card in Alice's hand
    let playable_card = state
        .player_hand
        .iter()
        .find(|c| c.is_playable)
        .expect("Alice must have at least one playable card in opening hand")
        .clone();

    let (events, next_state) = bridge
        .dispatch(GameCommand::PlayCard {
            card_id: playable_card.uid,
            target_id: None,
        })
        .expect("PlayCard must succeed");

    // Verify ManaSpent event is emitted and mana is deducted
    assert_eq!(next_state.mana[0], 6 - playable_card.cost);
    assert!(events.iter().any(|ev| matches!(
        ev,
        VisualEvent::ManaSpent {
            seat: 0,
            amount,
            remaining
        } if *amount == playable_card.cost && *remaining == 6 - playable_card.cost
    )));

    // Verify UnitSummoned or SpellCast event is emitted
    assert!(events.iter().any(|ev| matches!(
        ev,
        VisualEvent::UnitSummoned { card_uid, .. } | VisualEvent::SpellCast { card_uid, .. }
        if *card_uid == playable_card.uid
    )));
}

#[test]
fn test_animation_sequencer_blocking_and_concurrent_event_ordering() {
    let mut seq = AnimationSequencer::new();
    let mut board = VisualBoardState::default();
    assert_eq!(board.mana[0], 6);

    // Enqueue a non-blocking event (ManaSpent) followed by two blocking events (UnitSummoned, ShieldRevealed)
    seq.enqueue(
        VisualEvent::ManaSpent {
            seat: 0,
            amount: 2,
            remaining: 4,
        },
        None,
    );
    seq.enqueue(
        VisualEvent::UnitSummoned {
            seat: 0,
            card_uid: 101,
            card_spec_id: "H3W-AL-011".to_string(),
            title: "Dodo, Last Witness".to_string(),
            cost: 1,
            card_type: "Character".to_string(),
            slot_index: 0,
        },
        None,
    );
    seq.enqueue(
        VisualEvent::ShieldRevealed {
            seat: 1,
            card_uid: 202,
            card_spec_id: "H3W-RA-001".to_string(),
            title: "Wind the Heart".to_string(),
            trigger_text: None,
        },
        None,
    );

    // First update pops non-blocking ManaSpent AND first blocking UnitSummoned, holding ShieldRevealed in queue!
    let started = seq.update(0.016, &mut board);
    assert_eq!(started.len(), 2);
    assert_eq!(board.mana[0], 4);
    assert!(seq.active_blocking.is_some());
    assert_eq!(seq.queue.len(), 1);

    // Advancing by 0.1s while UnitSummoned (duration 0.38s) is still blocking keeps ShieldRevealed in queue
    let started_mid = seq.update(0.10, &mut board);
    assert!(started_mid.is_empty());
    assert_eq!(seq.queue.len(), 1);

    // Advancing past UnitSummoned completion (0.40s) finishes UnitSummoned and immediately pops ShieldRevealed!
    let started_next = seq.update(0.40, &mut board);
    assert_eq!(started_next.len(), 1);
    assert!(matches!(
        started_next[0],
        VisualEvent::ShieldRevealed { card_uid: 202, .. }
    ));
    assert_eq!(seq.queue.len(), 0);
}

#[test]
fn test_hand_arc_bezier_hover_magnification_and_velocity_tilt() {
    let normal_tf = compute_hand_arc_transform(2, 5, false, false);
    let hover_tf = compute_hand_arc_transform(2, 5, false, true);

    // Hovered card scales up 1.3x, shifts vertically upward (smaller y), and snaps to z_order = 100
    assert!((hover_tf.scale.x - 1.3).abs() < 1e-4);
    assert!(hover_tf.position.y < normal_tf.position.y);
    assert_eq!(hover_tf.z_order, 100);
    assert_eq!(hover_tf.rotation, 0.0);

    // Test CardVisual velocity-based tilt and spring damping
    let (_, _, state) = GameBridge::new(42, 0).unwrap();
    let sample_card = state.player_hand[0].clone();
    let mut visual = CardVisual::new(sample_card, vec2(200.0, 900.0), normal_tf);
    visual.current_transform.rotation = 0.0;
    visual.is_dragging = true;
    visual.target_transform = CardTransform {
        position: vec2(900.0, 500.0),
        rotation: 0.0,
        scale: vec2(1.1, 1.1),
        tilt_25d: vec2(0.0, 0.0),
        z_order: 200,
    };

    visual.update(0.016, vec2(1200.0, -300.0));
    // Positive X velocity tilts the card clockwise and shifts the drop shadow
    assert!(visual.current_transform.rotation > 0.0);
    assert!(visual.current_transform.tilt_25d.x > 0.0);

    // Non-linear easing boundary checks
    assert!((ease_out_elastic(1.0, 1.3, 0.0) - 1.0).abs() < 1e-4);
    assert!((ease_out_elastic(1.0, 1.3, 1.0) - 1.3).abs() < 1e-4);
    assert!((ease_out_back_bezier(0.0, 1.0, 0.5)) > 1.0); // Overshoot curve exceeds 1.0 mid-flight
}

#[test]
fn test_camera_shake_perlin_decay_and_particle_emitter_pool() {
    let mut cam = CameraController::new();
    cam.trigger_shake(0.8, 0.4);
    assert!(cam.trauma >= 0.8);

    cam.update(0.05);
    assert!(cam.current_offset.length() > 0.0 || cam.current_rotation_deg.abs() > 0.0);

    // After full duration, trauma decays back to 0 and offset resets to zero
    for _ in 0..30 {
        cam.update(0.05);
    }
    assert_eq!(cam.trauma, 0.0);
    assert_eq!(cam.current_offset, vec2(0.0, 0.0));

    // Particle pool allocation and expiration
    let mut vfx = VfxManager::new(128);
    vfx.spawn_minion_dust_cloud(vec2(960.0, 600.0));
    vfx.spawn_mana_crystal_shatter(vec2(500.0, 750.0), macroquad::prelude::BLUE);
    let active_before = vfx.pool.iter().filter(|p| p.active).count();
    assert!(active_before >= 50);

    for _ in 0..40 {
        vfx.update(0.05);
    }
    let active_after = vfx.pool.iter().filter(|p| p.active).count();
    assert_eq!(active_after, 0);
}

#[test]
fn test_full_turn_cycle_and_end_turn_ai_response() {
    let (mut bridge, _, state) = GameBridge::new(42, 0).unwrap();
    let card_uid = state.player_hand.iter().find(|c| c.is_playable).unwrap().uid;

    // Play a card, resolve sequence, and end turn (which runs White Rabbit AI turn and starts Turn 3)
    bridge
        .dispatch(GameCommand::PlayCard {
            card_id: card_uid,
            target_id: None,
        })
        .unwrap();
    bridge.dispatch(GameCommand::ResolveSequence).unwrap();
    let (end_events, after_ai_state) = bridge.dispatch(GameCommand::EndTurn).unwrap();

    assert!(!end_events.is_empty());
    assert!(after_ai_state.turn_number >= 2);
    assert_eq!(after_ai_state.active_seat, 0);
}

#[test]
fn test_all_44_cards_ascii_typography_normalization() {
    let specs = heav3ns_engine::engine::load_proposed_specs();
    let mut g = heav3ns_engine::engine::Game::new(Some(specs.clone()), false).unwrap();
    for s in &specs {
        let cid = s.get("id").and_then(|v| v.as_str()).unwrap();
        let uid = g.add(0, cid, "hand").unwrap();
        let card = g.get_card(uid).unwrap();
        let vcard = client_macroquad::bridge::VisualCardData::from_engine_card(&card, true);

        // Ensure no unrenderable Unicode curly quotes or em-dashes remain in title or text
        assert!(
            vcard.title.is_ascii(),
            "Card title not normalized to ASCII: {}",
            vcard.title
        );
        assert!(
            vcard.text.is_ascii(),
            "Card text not normalized to ASCII for {}: {}",
            vcard.spec_id,
            vcard.text
        );
        if let Some(ref st) = vcard.shield_trigger {
            assert!(
                st.is_ascii(),
                "Card shield_trigger not normalized to ASCII: {st}"
            );
        }
    }
}

#[test]
fn test_interactive_modal_prompts_for_choice_pick_and_discard() {
    let (mut bridge, _, _) = GameBridge::new(42, 0).unwrap();
    bridge.interactive_prompts = true;

    // 1. Test modal `choice` prompt on H3W-AL-001 (A Question of Doors: draw1 vs bounce_friendly)
    // Add a friendly character on board so both draw1 and bounce_friendly are viable choices
    let dodo_uid = {
        bridge.game.phase = "ended".to_string();
        let u = bridge.game.add(0, "H3W-AL-011", "field").unwrap();
        let doors = bridge.game.add(0, "H3W-AL-001", "hand").unwrap();
        let fish = bridge.game.add(0, "H3W-AL-012", "hand").unwrap();
        bridge.game.phase = "placement".to_string();
        (u, doors, fish)
    };
    let (_dodo, doors_uid, fish_uid) = dodo_uid;

    // Play A Question of Doors and resolve sequence -> should pause and emit ActivePrompt (kind == "choice")
    bridge
        .dispatch(GameCommand::PlayCard {
            card_id: doors_uid,
            target_id: None,
        })
        .unwrap();
    let (_, paused_state) = bridge.dispatch(GameCommand::ResolveSequence).unwrap();
    let prompt = paused_state
        .prompt
        .expect("Expected interactive choice prompt for A Question of Doors");
    assert_eq!(prompt.kind, "choice");
    assert!(prompt.options.len() >= 2);
    assert!(prompt.title.is_ascii());

    // Submit decision selecting option 0 (draw1) -> completes sequence resolution!
    let (res_events, resolved_state) = bridge
        .dispatch(GameCommand::SubmitDecision {
            option_index: Some(0),
            selected_uids: Vec::new(),
        })
        .unwrap();
    assert!(resolved_state.prompt.is_none());
    assert_eq!(resolved_state.phase, "resolved");
    assert!(res_events
        .iter()
        .any(|ev| matches!(ev, VisualEvent::CardDrawn { seat: 0, .. })));

    // 2. Test interactive `pick` prompt on H3W-AL-012 (The Fish Footman: look at top 3, choose 1)
    bridge.game.p[0].actions = 6;
    bridge
        .dispatch(GameCommand::PlayCard {
            card_id: fish_uid,
            target_id: None,
        })
        .unwrap();
    let (_, pick_paused_state) = bridge.dispatch(GameCommand::ResolveSequence).unwrap();
    let pick_prompt = pick_paused_state
        .prompt
        .expect("Expected interactive pick prompt for The Fish Footman");
    assert_eq!(pick_prompt.kind, "pick");
    assert_eq!(pick_prompt.options.len(), 3);
    let chosen_pick_uid = pick_prompt.options[1].target_uid.unwrap();

    // Submit decision picking the second revealed card
    let (pick_events, after_pick_state) = bridge
        .dispatch(GameCommand::SubmitDecision {
            option_index: Some(1),
            selected_uids: vec![chosen_pick_uid],
        })
        .unwrap();
    assert!(after_pick_state.prompt.is_none());
    assert!(after_pick_state
        .player_hand
        .iter()
        .any(|c| c.uid == chosen_pick_uid));
    assert!(pick_events.iter().any(|ev| matches!(
        ev,
        VisualEvent::CardDrawn {
            seat: 0,
            card_uid: Some(u),
            ..
        } if *u == chosen_pick_uid
    )));
}

#[test]
fn test_interactive_counter_during_opponent_turn_and_clean_transitions() {
    let (mut bridge, _, _) = GameBridge::new(42, 0).unwrap();
    bridge.interactive_prompts = true;

    // Give Alice H3W-AL-004 (The Raven's Footnote, Cost 1, Counter: cancel action/counter),
    // keep Alice's hand at 5 cards (moving 1 card to discard so card conservation holds),
    // and move White Rabbit's starting hand to discard before adding H3W-RA-001 (Action) so
    // White Rabbit is guaranteed to play an Action card on its turn!
    let raven_uid = {
        bridge.game.phase = "ended".to_string();
        if let Some(c) = bridge.game.p[0].hand.pop() {
            bridge.game.p[0].discard.push(c);
        }
        while let Some(c) = bridge.game.p[1].hand.pop() {
            bridge.game.p[1].discard.push(c);
        }
        let r = bridge.game.add(0, "H3W-AL-004", "hand").unwrap();
        let _rabbit_act = bridge.game.add(1, "H3W-RA-001", "hand").unwrap();
        bridge.game.phase = "placement".to_string();
        r
    };

    // Alice ends her turn with 6 Actions remaining so she can afford The Raven's Footnote during Rabbit's turn.
    let (_, paused_ai_state) = bridge.dispatch(GameCommand::EndTurn).unwrap();

    // Verify that White Rabbit's turn paused on Alice's interactive Counter/Response window!
    let resp_prompt = paused_ai_state
        .prompt
        .expect("Expected interactive response prompt during White Rabbit's turn");
    assert_eq!(resp_prompt.kind, "response");
    assert!(resp_prompt.options.len() >= 2);
    assert!(resp_prompt
        .options
        .iter()
        .any(|o| o.target_uid == Some(raven_uid)));

    // Verify no rejected transition records polluted `bridge.game.transitions` while pausing for PROMPT_REQUIRED
    assert!(
        bridge
            .game
            .transitions
            .iter()
            .all(|tr| tr["status"] == serde_json::json!("committed")),
        "PROMPT_REQUIRED must not leave rejected transition records in game.transitions"
    );

    // Choose the option that plays The Raven's Footnote to counter White Rabbit's action
    let raven_opt_idx = resp_prompt
        .options
        .iter()
        .position(|o| o.target_uid == Some(raven_uid))
        .unwrap();
    let (mut all_events, mut cur_state) = bridge
        .dispatch(GameCommand::SubmitDecision {
            option_index: Some(raven_opt_idx),
            selected_uids: Vec::new(),
        })
        .unwrap();

    // If additional prompts occur during the remainder of White Rabbit's turn, pass/resolve them
    let mut safety = 0;
    while cur_state.prompt.is_some() && safety < 20 {
        safety += 1;
        let (more_ev, next_s) = bridge
            .dispatch(GameCommand::SubmitDecision {
                option_index: Some(0),
                selected_uids: Vec::new(),
            })
            .unwrap();
        all_events.extend(more_ev);
        cur_state = next_s;
    }

    assert!(cur_state.prompt.is_none());
    assert_eq!(cur_state.active_seat, 0);
    // Verify Alice's Counter was played and cancelled White Rabbit's effect
    assert!(all_events.iter().any(|ev| matches!(
        ev,
        VisualEvent::SpellCast {
            seat: 0,
            card_uid,
            ..
        } if *card_uid == raven_uid
    )));
    assert!(all_events
        .iter()
        .any(|ev| matches!(ev, VisualEvent::EffectCancelled { .. })));
    assert!(
        bridge
            .game
            .transitions
            .iter()
            .all(|tr| tr["status"] == serde_json::json!("committed")),
        "All recorded transitions must be committed"
    );
}

#[test]
fn test_nested_lifo_counter_reply_chain_and_multicard_discard_during_interactive_match() {
    let (mut bridge, _, _) = GameBridge::new(42, 0).unwrap();
    bridge.interactive_prompts = true;

    // Setup a 3-level LIFO Counter-to-Counter chain during White Rabbit's turn:
    // - Alice has two Counters in hand: H3W-AL-004 (The Raven's Footnote, Cost 1) and H3W-AL-007 (The Queen's Favor, Cost 2)
    // - White Rabbit has H3W-RA-008 (Action, Cost 2) AND H3W-RA-009 (The Unwritten Hour, Counter, Cost 1)
    let (raven_uid, mouse_uid) = {
        bridge.game.phase = "ended".to_string();
        for _ in 0..2 {
            if let Some(c) = bridge.game.p[0].hand.pop() {
                bridge.game.p[0].discard.push(c);
            }
        }
        while let Some(c) = bridge.game.p[1].hand.pop() {
            bridge.game.p[1].discard.push(c);
        }
        let r = bridge.game.add(0, "H3W-AL-004", "hand").unwrap();
        let m = bridge.game.add(0, "H3W-AL-007", "hand").unwrap();
        let _ra_act = bridge.game.add(1, "H3W-RA-008", "hand").unwrap();
        let _ra_cnt = bridge.game.add(1, "H3W-RA-009", "hand").unwrap();
        bridge.game.phase = "placement".to_string();
        bridge.game.transitions.clear();
        (r, m)
    };
    let initial_snap = heav3ns_engine::contract::snapshot(&bridge.game).unwrap();

    // Also set a stale `preferred_target` to prove it does NOT hijack multi-step interactive prompts
    bridge.preferred_target = Some(raven_uid);

    // 1. Alice ends her turn -> White Rabbit plays H3W-RA-008 -> Alice gets first Response prompt (is_reply = false)
    let (_, state1) = bridge.dispatch(GameCommand::EndTurn).unwrap();
    let prompt1 = state1
        .prompt
        .expect("Expected initial response prompt against White Rabbit's action");
    assert_eq!(prompt1.kind, "response");

    // Alice chooses The Raven's Footnote (`raven_uid`)
    let idx1 = prompt1
        .options
        .iter()
        .position(|o| o.target_uid == Some(raven_uid))
        .unwrap();
    let (_, state2) = bridge
        .dispatch(GameCommand::SubmitDecision {
            option_index: Some(idx1),
            selected_uids: Vec::new(),
        })
        .unwrap();

    // 2. White Rabbit replies with its Counter (H3W-RA-009), opening a nested reply window (is_reply = true) for Alice!
    let prompt2 = state2
        .prompt
        .expect("Expected nested LIFO reply prompt after White Rabbit countered Alice's Counter");
    assert_eq!(prompt2.kind, "response");
    let idx2 = prompt2
        .options
        .iter()
        .position(|o| o.target_uid == Some(mouse_uid))
        .expect("Alice's second Counter (The Queen's Favor) must be offered in nested reply window");

    // Alice plays her second Counter (`mouse_uid`) to cancel White Rabbit's Counter!
    let (mut events, mut cur_state) = bridge
        .dispatch(GameCommand::SubmitDecision {
            option_index: Some(idx2),
            selected_uids: Vec::new(),
        })
        .unwrap();

    while cur_state.prompt.is_some() {
        let (more_ev, next_s) = bridge
            .dispatch(GameCommand::SubmitDecision {
                option_index: Some(0),
                selected_uids: Vec::new(),
            })
            .unwrap();
        events.extend(more_ev);
        cur_state = next_s;
    }

    assert_eq!(cur_state.active_seat, 0);
    // Both of Alice's Counters and White Rabbit's Counter were cast, and 2 effects were cancelled in LIFO order
    let cancel_count = events
        .iter()
        .filter(|e| matches!(e, VisualEvent::EffectCancelled { .. }))
        .count();
    assert_eq!(cancel_count, 2, "Expected both Rabbit's Counter and Rabbit's Action to be cancelled");

    // 3. Test multi-card discard prompt (8 cards in hand -> must discard 2) submitted via OptionIndex(0) (e.g., Spacebar)
    // Use `draw` command on the engine to give Alice 8 cards in hand so transition replay stays 100% legal!
    let needed_draws = (8usize.saturating_sub(bridge.game.p[0].hand.len())) as i32;
    if needed_draws > 0 {
        bridge.game.draw(0, needed_draws).unwrap();
    }
    bridge.dispatch(GameCommand::ResolveSequence).unwrap();

    let (_, disc_paused) = bridge.dispatch(GameCommand::EndTurn).unwrap();
    let disc_prompt = disc_paused
        .prompt
        .expect("Expected multi-card discard prompt when hand size is 8");
    assert_eq!(disc_prompt.kind, "discard");
    assert_eq!(disc_prompt.required_cards, 2);

    // Submitting OptionIndex(0) with empty selected_uids (Spacebar shortcut) must cleanly discard 2 cards without error!
    let (_, mut after_disc) = bridge
        .dispatch(GameCommand::SubmitDecision {
            option_index: Some(0),
            selected_uids: Vec::new(),
        })
        .unwrap();
    while after_disc.prompt.is_some() {
        let (_, next_s) = bridge
            .dispatch(GameCommand::SubmitDecision {
                option_index: Some(0),
                selected_uids: Vec::new(),
            })
            .unwrap();
        after_disc = next_s;
    }
    assert!(after_disc.prompt.is_none());

    // Verify all transitions in `bridge.game.transitions` are committed and replayable via `replay_transitions`
    assert!(bridge
        .game
        .transitions
        .iter()
        .all(|tr| tr["status"] == serde_json::json!("committed")));
    let specs = heav3ns_engine::engine::load_proposed_specs();
    let replayed = heav3ns_engine::contract::replay_transitions(
        specs,
        &initial_snap,
        &serde_json::Value::Array(bridge.game.transitions.clone()),
    )
    .expect("Transition log with interactive prompts must replay deterministically without divergence");
    assert_eq!(replayed.log, bridge.game.log);
}


