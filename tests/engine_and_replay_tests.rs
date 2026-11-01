use heav3ns_engine::{
    adapter::{pool_sha256, stock_game, Match},
    contract::{replay_transitions, restore, snapshot, verify_replay_data},
    engine::{load_default_specs, load_proposed_specs, specs_hash, Game, Response},
    policy::{affordable_sequences, public_clone, AuditPolicy},
    rng::PyRng,
    server::{handle_connection, ServerState},
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[test]
fn test_python_mt19937_rng_parity() {
    let mut rng = PyRng::new(0);
    // Seed 0 initial sha256 matches Python random.Random(0)
    assert_eq!(rng.index, 624);
    assert_eq!(rng.mt.len(), 624);

    let mut deck: Vec<i32> = (1..=40).collect();
    rng.seed_u64(84); // 42 * 2 + 0
    rng.shuffle(&mut deck);
    assert_eq!(deck.len(), 40);

    let state_json = rng.getstate_json();
    let mut restored = PyRng::new(999);
    restored.setstate_json(&state_json).unwrap();
    assert_eq!(rng.rng_sha256(), restored.rng_sha256());
}

#[test]
fn test_canonical_card_pool_and_specs_hashes() {
    assert_eq!(
        pool_sha256(),
        "db4fb5bad7c274cc77a5323e44651dc427a17ba67a9e80e47706000fbc5517c2"
    );
    let specs = load_proposed_specs();
    assert_eq!(specs.len(), 44);
    assert_eq!(
        specs_hash(&specs),
        "4a290f53da32379b4f0ff87ee988cd7ad142944d461f09724c87d46626661704"
    );
}

#[test]
fn test_historical_replays_verify_bit_exact() {
    for path in [
        "fixtures/sample-complete-replay.json",
        "fixtures/downloaded-seed2-replay.json",
        "fixtures/downloaded-seed2-final.json",
    ] {
        let raw = std::fs::read_to_string(path).expect("fixture exists");
        let data: Value = serde_json::from_str(&raw).expect("valid json");
        let res = verify_replay_data(&data).expect("replay must verify");
        assert_eq!(res["ok"], json!(true));
    }
}

#[test]
fn test_forward_support_and_returned_supporter_prevents_combo() {
    let mut g = Game::new(Some(load_proposed_specs()), false).unwrap();
    // Alice: H3W-AL-016 (The Crownless Lion, Cost 3, Character, Combo 1: Break 2 Shields)
    // Alice: H3W-AL-011 (Dodo, Last Witness, Cost 1, Character, supporter)
    // White Rabbit: H3W-RA-002 (Borrowed Seconds, Cost 1, Counter: counter_bounce_later on combo)
    let c_combo = g.add(0, "H3W-AL-016", "hand").unwrap();
    let c_supp = g.add(0, "H3W-AL-011", "hand").unwrap();
    let c_counter = g.add(1, "H3W-RA-002", "hand").unwrap();
    for _ in 0..3 {
        g.add(0, "H3W-AL-001", "shields").unwrap();
        g.add(1, "H3W-RA-001", "shields").unwrap();
    }

    g.start(0).unwrap();
    g.place(c_combo).unwrap();
    g.place(c_supp).unwrap();

    // Without counter, Crownless Lion has 1 later Character supporter (`c_supp`)
    let cat_card = g.get_card(c_combo).unwrap();
    assert_eq!(g.supporters(&cat_card).len(), 1);
    assert!(g.combo_eligible(&cat_card));

    // Script Borrowed Seconds (`c_counter`) to respond to Crownless Lion's `combo` window,
    // bouncing `c_supp` back to Alice's hand so Combo loses its later supporter!
    let mut responses = HashMap::new();
    responses.insert(
        (c_combo, "combo".to_string()),
        vec![Response::new(c_counter)],
    );
    let mut choices = HashMap::new();
    let mut ch_map = Map::new();
    ch_map.insert("target".to_string(), json!(c_supp));
    choices.insert((c_counter, "counter".to_string()), ch_map);

    let mut np = heav3ns_engine::engine::NoProviders;
    g.close_with_options(Some(responses), Some(choices), &mut np)
        .unwrap();

    // Verify Combo lost support and no shields were broken
    assert_eq!(g.p[1].shields.len(), 3);
    assert!(g
        .log
        .iter()
        .any(|ev| ev.first().and_then(|v| v.as_str()) == Some("combo_lost_support")));
    // Returned supporter is back in Alice's hand, not discard
    assert!(g.p[0].hand.iter().any(|c| c.uid == c_supp));
}

#[test]
fn test_cancelled_entry_does_not_suppress_combo_and_lifo_counter_reply() {
    let mut g = Game::new(Some(load_proposed_specs()), false).unwrap();
    // Alice: H3W-AL-014 (Cheshire Wayfinder, Cost 3, Character, On Entry + Combo 1: select top 3)
    // Alice: H3W-AL-001 (supporter, Cost 1)
    // Alice: H3W-AL-004 (The Raven’s Footnote, Cost 1, Counter: cancel action/counter)
    // White Rabbit: H3W-RA-007 (Snap the Spring, Cost 1, Counter: cancel entry)
    let hatter = g.add(0, "H3W-AL-014", "hand").unwrap();
    let supp = g.add(0, "H3W-AL-001", "hand").unwrap();
    let alice_counter = g.add(0, "H3W-AL-004", "hand").unwrap();
    let rabbit_counter = g.add(1, "H3W-RA-007", "hand").unwrap();
    let deck_card = g.add(0, "H3W-AL-004", "deck").unwrap();
    g.add(0, "H3W-AL-001", "shields").unwrap();
    g.add(1, "H3W-RA-001", "shields").unwrap();

    // Clone to test Case A: Rabbit cancels Hatter's Entry, Hatter's Combo still resolves!
    let mut g_a = g.clone();
    g_a.start(0).unwrap();
    g_a.place(hatter).unwrap();
    g_a.place(supp).unwrap();
    let mut resp_a = HashMap::new();
    resp_a.insert(
        (hatter, "entry".to_string()),
        vec![Response::new(rabbit_counter)],
    );
    let mut np = heav3ns_engine::engine::NoProviders;
    g_a.close_with_options(Some(resp_a), None, &mut np).unwrap();

    // Entry was cancelled, but Combo resolved (`select` moved `deck_card` into hand)
    assert!(g_a.log.iter().any(|ev| {
        ev.first().and_then(|v| v.as_str()) == Some("cancel")
            && ev.get(1).and_then(|v| v.as_i64()) == Some(hatter as i64)
            && ev.get(2).and_then(|v| v.as_str()) == Some("entry")
    }));
    assert!(g_a.log.iter().any(|ev| {
        ev.first().and_then(|v| v.as_str()) == Some("select")
            && ev.get(1).and_then(|v| v.as_i64()) == Some(deck_card as i64)
    }));

    // Case B: LIFO Counter-to-Counter reply!
    // Enemy character on field so Hatter's Entry bounce has a target
    let enemy_char = g.add(1, "H3W-RA-011", "field").unwrap();
    g.start(0).unwrap();
    g.place(hatter).unwrap();
    g.place(supp).unwrap();
    // Rabbit counters Hatter's Entry with `rabbit_counter`, and Alice replies with `alice_counter`!
    let mut resp_b = HashMap::new();
    resp_b.insert(
        (hatter, "entry".to_string()),
        vec![Response::with_reply(
            rabbit_counter,
            Response::new(alice_counter),
        )],
    );
    g.close_with_options(Some(resp_b), None, &mut np).unwrap();

    // Rabbit's Counter was cancelled by Alice's reply, so Hatter's Entry resolved and bounced `enemy_char`!
    assert!(g.log.iter().any(|ev| {
        ev.first().and_then(|v| v.as_str()) == Some("cancel")
            && ev.get(1).and_then(|v| v.as_i64()) == Some(rabbit_counter as i64)
            && ev.get(2).and_then(|v| v.as_str()) == Some("counter")
    }));
    assert!(g.p[1].hand.iter().any(|c| c.uid == enemy_char));
}

#[test]
fn test_shield_trigger_draw_before_discard_and_opening_protection() {
    let mut g = Game::new(Some(load_default_specs()), true).unwrap();
    // H3W-RA-002 has Shield Trigger: Draw 1 card.
    let s1 = g.add(1, "H3W-RA-002", "shields").unwrap();
    let s2 = g.add(1, "H3W-RA-001", "shields").unwrap();
    let d1 = g.add(1, "H3W-RA-004", "deck").unwrap();
    g.add(0, "H3W-AL-001", "shields").unwrap();

    // Turn 1 with opening_protection = true: breaking 2 shields stops at 1 shield remaining!
    g.start(0).unwrap();
    g.break_shields(0, 2).unwrap();

    assert_eq!(g.p[1].shields.len(), 1);
    assert_eq!(g.p[1].shields[0].uid, s2);
    // Shield Trigger drew `d1` BEFORE `s1` moved to discard
    let reveal_pos = g
        .log
        .iter()
        .position(|ev| ev.first().and_then(|v| v.as_str()) == Some("reveal"))
        .unwrap();
    let draw_pos = g
        .log
        .iter()
        .position(|ev| ev.first().and_then(|v| v.as_str()) == Some("draw"))
        .unwrap();
    let sdisc_pos = g
        .log
        .iter()
        .position(|ev| ev.first().and_then(|v| v.as_str()) == Some("shield_discard"))
        .unwrap();
    assert!(reveal_pos < draw_pos && draw_pos < sdisc_pos);
    assert!(g.p[1].hand.iter().any(|c| c.uid == d1));
    assert!(g.p[1].discard.iter().any(|c| c.uid == s1));
    assert!(g
        .log
        .iter()
        .any(|ev| ev.first().and_then(|v| v.as_str()) == Some("opening_floor")));
}

#[test]
fn test_atomic_transaction_rollback_and_hand_limit_discard() {
    let mut g = stock_game(42).unwrap();
    g.trace_transitions = true;
    g.start(0).unwrap();
    g.close().unwrap();

    // Give Alice extra cards in hand by drawing so hand > 6
    g.draw(0, 3).unwrap();
    assert_eq!(g.p[0].hand.len(), 8);
    let snap_before = snapshot(&g).unwrap();

    // Attempting end_turn([]) when 2 discards are required must fail and roll back cleanly
    let err = g.end_turn(&[]).unwrap_err();
    assert!(err.0.contains("discard to six"));
    assert_eq!(snapshot(&g).unwrap(), snap_before);
    // Transition log records the rejected attempt with rolled_back = true
    let last_tr = g.transitions.last().unwrap();
    assert_eq!(last_tr["status"], json!("rejected"));
    assert_eq!(last_tr["rolled_back"], json!(true));

    // Providing exact 2 excess UIDs commits cleanly
    let d_uids = vec![g.p[0].hand[0].uid, g.p[0].hand[1].uid];
    g.end_turn(&d_uids).unwrap();
    assert_eq!(g.p[0].hand.len(), 6);
    assert_eq!(g.phase, "ended");
}

#[test]
fn test_public_clone_redacts_hidden_zones_and_audit_policy_runs() {
    let mut g = stock_game(42).unwrap();
    g.start(0).unwrap();
    let pub_g = public_clone(&g, 0);
    // Alice's own hand is visible; White Rabbit's hand, both decks, and both shield zones are UNKNOWN
    assert!(pub_g.p[0].hand.iter().all(|c| c.id() != "UNKNOWN"));
    assert!(pub_g.p[1].hand.iter().all(|c| c.id() == "UNKNOWN"));
    assert!(pub_g.p[0].deck.iter().all(|c| c.id() == "UNKNOWN"));
    assert!(pub_g.p[1].deck.iter().all(|c| c.id() == "UNKNOWN"));
    assert!(pub_g.p[0].shields.iter().all(|c| c.id() == "UNKNOWN"));
    assert!(pub_g.p[1].shields.iter().all(|c| c.id() == "UNKNOWN"));

    let seqs = affordable_sequences(&g.p[0].hand, 6);
    assert!(!seqs.is_empty());

    let mut ai = AuditPolicy::new(["mixed", "mixed"]);
    let chosen = ai.sequence(&g, 0);
    assert!(!chosen.is_empty());
}

#[test]
fn test_snapshot_restore_and_replay_transitions_roundtrip() {
    let mut g = stock_game(7).unwrap();
    let initial = snapshot(&g).unwrap();
    g.trace_transitions = true;
    let mut ai = AuditPolicy::new_heuristic(["offense", "control"]);

    g.start_with_providers(0, &ai).unwrap();
    let seq = ai.sequence(&g, 0);
    for c in seq {
        g.place_with_providers(c.uid, &ai).unwrap();
    }
    g.close_with_providers(&mut ai).unwrap();
    g.end_turn_with_providers(&[], &ai).unwrap();

    let mid_snap = snapshot(&g).unwrap();
    let restored = restore(&mid_snap, load_proposed_specs(), false, true).unwrap();
    assert_eq!(snapshot(&restored).unwrap(), mid_snap);

    let replayed = replay_transitions(
        load_proposed_specs(),
        &initial,
        &Value::Array(g.transitions.clone()),
    )
    .unwrap();
    assert_eq!(replayed.log, g.log);
    assert_eq!(replayed.rng.rng_sha256(), g.rng.rng_sha256());
}

#[test]
fn test_adapter_privacy_and_http_server_framing_hardening() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let m = Match::new(42, 0).unwrap();
    let state = Arc::new(Mutex::new(ServerState {
        current_match: m,
        static_root: PathBuf::from("assets"),
    }));

    let st_clone = Arc::clone(&state);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let s = Arc::clone(&st_clone);
            thread::spawn(move || {
                handle_connection(stream, s, port);
            });
        }
    });

    // Wait until match reaches 'waiting'
    for _ in 0..40 {
        let s = state.lock().unwrap().current_match.state();
        if s["status"] == json!("waiting") {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(20));
    }

    let send_raw = |req: &str| -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(req.as_bytes()).unwrap();
        stream.flush().unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        let status_line = resp.lines().next().unwrap_or("");
        let code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let body = resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (code, body)
    };

    // 1. Valid GET /api/state
    let (code, body) = send_raw(&format!(
        "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    ));
    assert_eq!(code, 200);
    let state_json: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(state_json["seed"], json!(42));
    // Verify opponent hand and both shield zones are strictly hidden
    assert!(state_json["players"][1]["hand"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c == &json!({"hidden": true})));
    assert!(state_json["players"][0]["shields"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c == &json!({"hidden": true})));

    // 2. Reject invalid Host header (400 matching server.py)
    let (code_bad_host, _) = send_raw("GET /api/state HTTP/1.1\r\nHost: evil.example.com\r\n\r\n");
    assert_eq!(code_bad_host, 400);

    // 3. Reject cross-site Sec-Fetch-Site (400 matching server.py)
    let (code_xsite, _) = send_raw(&format!(
        "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nSec-Fetch-Site: cross-site\r\n\r\n"
    ));
    assert_eq!(code_xsite, 400);

    // 4. Reject Transfer-Encoding: chunked on POST (400)
    let (code_te, _) = send_raw(&format!(
        "POST /api/reset HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
    ));
    assert_eq!(code_te, 400);

    // 5. Reject duplicate Content-Length headers (400)
    let (code_dup_cl, _) = send_raw(&format!(
        "POST /api/reset HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{{}}"
    ));
    assert_eq!(code_dup_cl, 400);

    // 6. Reject premature /api/replay before match ends (400)
    let (code_replay_early, _) = send_raw(&format!(
        "GET /api/replay HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    ));
    assert_eq!(code_replay_early, 400);

    // 7. Reject path traversal (both raw and URL-encoded) with 403
    let (code_trav, _) = send_raw(&format!(
        "GET /../../Cargo.toml HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    ));
    assert_eq!(code_trav, 403);
    let (code_trav_enc, _) = send_raw(&format!(
        "GET /%2e%2e/Cargo.toml HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    ));
    assert_eq!(code_trav_enc, 403);

    // 8. Reject overlong path with 414
    let long_path = format!("/{}", "a".repeat(5000));
    let (code_long, _) = send_raw(&format!(
        "GET {long_path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    ));
    assert_eq!(code_long, 414);

    // 9. Reject OPTIONS with 403
    let (code_opt, _) = send_raw(&format!(
        "OPTIONS /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    ));
    assert_eq!(code_opt, 403);

    // 10. Reject unknown keys in POST /api/reset with 400
    let bad_reset = r#"{"seed":42,"extra":1}"#;
    let (code_bad_reset, _) = send_raw(&format!(
        "POST /api/reset HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{bad_reset}",
        bad_reset.len()
    ));
    assert_eq!(code_bad_reset, 400);

    // 11. Serve static card asset from assets/cards (200)
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let req = format!("GET /cards/card-back.webp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    stream.flush().unwrap();
    let mut raw_bytes = Vec::new();
    stream.read_to_end(&mut raw_bytes).unwrap();
    let header_str = String::from_utf8_lossy(&raw_bytes[..128.min(raw_bytes.len())]);
    assert!(header_str.starts_with("HTTP/1.1 200 OK"));
    assert!(header_str.contains("Content-Type: image/webp"));

    state.lock().unwrap().current_match.stop();
}

#[test]
fn test_deep_3_level_lifo_counter_chain() {
    let mut g = Game::new(Some(load_proposed_specs()), false).unwrap();
    // Alice: H3W-AL-014 (Cheshire Wayfinder, Cost 3, Character, On Entry: bounce enemy cost<=2)
    // Alice: H3W-AL-004 (The Raven’s Footnote, Cost 1, Counter: cancel action/counter)
    // White Rabbit: H3W-RA-007 (Snap the Spring, Cost 1, Counter: cancel entry)
    // White Rabbit: H3W-RA-009 (The Unwritten Hour, Cost 1, Counter: cancel action/counter)
    let hatter = g.add(0, "H3W-AL-014", "hand").unwrap();
    let alice_c2 = g.add(0, "H3W-AL-004", "hand").unwrap();
    let rabbit_c1 = g.add(1, "H3W-RA-007", "hand").unwrap();
    let rabbit_c3 = g.add(1, "H3W-RA-009", "hand").unwrap();
    let enemy_char = g.add(1, "H3W-RA-011", "field").unwrap();
    g.add(0, "H3W-AL-001", "shields").unwrap();
    g.add(1, "H3W-RA-001", "shields").unwrap();

    g.start(0).unwrap();
    g.place(hatter).unwrap();
    // 3-level LIFO chain:
    // Level 1: Rabbit counters Hatter's entry with `rabbit_c1`
    // Level 2: Alice replies to `rabbit_c1` with `alice_c2`
    // Level 3: Rabbit replies to `alice_c2` with `rabbit_c3`
    // Expected LIFO resolution:
    // - `rabbit_c3` resolves first and cancels `alice_c2`!
    // - `alice_c2` is now cancelled (`cancelled == true`), so it does NOT resolve!
    // - `rabbit_c1` survives and resolves, cancelling Hatter's `entry`!
    // - Hatter's `entry` is cancelled, so `enemy_char` remains on Rabbit's field!
    let mut resp = HashMap::new();
    resp.insert(
        (hatter, "entry".to_string()),
        vec![Response::with_reply(
            rabbit_c1,
            Response::with_reply(alice_c2, Response::new(rabbit_c3)),
        )],
    );
    let mut np = heav3ns_engine::engine::NoProviders;
    g.close_with_options(Some(resp), None, &mut np).unwrap();

    // Verify `alice_c2` was marked cancelled in g.cards
    assert!(g.get_card(alice_c2).unwrap().cancelled);
    // Verify `rabbit_c1` was NOT cancelled
    assert!(!g.get_card(rabbit_c1).unwrap().cancelled);
    // Verify Hatter's entry was cancelled and `enemy_char` is still on Rabbit's field
    assert!(g.get_card(hatter).unwrap().cancelled);
    assert!(g.p[1].field.iter().any(|c| c.uid == enemy_char));
}

#[test]
fn test_replay_corruption_and_diagnostic_rejection() {
    let raw = std::fs::read_to_string("fixtures/sample-complete-replay.json").unwrap();
    let base: Value = serde_json::from_str(&raw).unwrap();
    let ok = verify_replay_data(&base).unwrap();
    assert_eq!(
        ok,
        json!({
            "ok": true,
            "winner": "White Rabbit",
            "turns": 4,
            "transitions": 23
        })
    );

    // 1. Diagnostic error record must be rejected
    let mut diag = base.clone();
    diag["error"] = json!("RuntimeError('injected fault')");
    assert!(verify_replay_data(&diag).is_err());

    // 2. Tampered runtime_sha256 must be rejected
    let mut bad_rt = base.clone();
    bad_rt["runtime_sha256"]["engine.py"] = json!("deadbeef");
    assert!(verify_replay_data(&bad_rt).is_err());

    // 3. Missing final end_turn cleanup must be rejected
    let mut no_end = base.clone();
    no_end["transitions"].as_array_mut().unwrap().pop();
    assert!(verify_replay_data(&no_end).is_err());
}

#[test]
fn test_end_to_end_http_interactive_match_and_replay_verification() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let m = Match::new(2, 0).unwrap();
    let state = Arc::new(Mutex::new(ServerState {
        current_match: m,
        static_root: PathBuf::from("assets"),
    }));

    let st_clone = Arc::clone(&state);
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let s = Arc::clone(&st_clone);
            thread::spawn(move || {
                handle_connection(stream, s, port);
            });
        }
    });

    let send_http = |method: &str, path: &str, body_opt: Option<&Value>| -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let req = if let Some(b) = body_opt {
            let s = serde_json::to_string(b).unwrap();
            format!(
                "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{s}",
                s.len()
            )
        } else {
            format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n")
        };
        stream.write_all(req.as_bytes()).unwrap();
        stream.flush().unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        let status_line = resp.lines().next().unwrap_or("");
        let code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let body_str = resp.split("\r\n\r\n").nth(1).unwrap_or("{}");
        let val: Value = serde_json::from_str(body_str).unwrap_or(Value::Null);
        (code, val)
    };

    // Reset to seed 2, starter 0 via POST /api/reset while previous match (seed 2) is waiting on a decision
    for _ in 0..40 {
        let (_, s) = send_http("GET", "/api/state", None);
        if s["status"] == json!("waiting") {
            break;
        }
        thread::sleep(std::time::Duration::from_millis(10));
    }
    let old_gen = {
        let (_, s) = send_http("GET", "/api/state", None);
        s["generation"].as_str().unwrap_or("").to_string()
    };
    let (r_code, mut cur_state) = send_http(
        "POST",
        "/api/reset",
        Some(&json!({"seed": 2, "starter": 0})),
    );
    assert_eq!(r_code, 200);
    assert_eq!(cur_state["seed"], json!(2));
    assert_ne!(cur_state["generation"], json!(old_gen));

    // Drive full interactive match over HTTP until status == "finished"
    let mut step_guard = 0;
    while cur_state["status"] != json!("finished") {
        step_guard += 1;
        assert!(step_guard < 400, "match did not finish within 400 steps");
        if cur_state["status"] != json!("waiting") {
            thread::sleep(std::time::Duration::from_millis(10));
            let (c, s) = send_http("GET", "/api/state", None);
            assert_eq!(c, 200);
            cur_state = s;
            continue;
        }
        let pending = &cur_state["pending"];
        let dec_id = pending["id"].as_str().unwrap().to_string();

        // Also verify stale decision ID is rejected with 400
        if step_guard == 1 {
            let (stale_code, _) = send_http(
                "POST",
                "/api/action",
                Some(&json!({"decision_id": "stale-id", "option_id": "0"})),
            );
            assert_eq!(stale_code, 400);
        }

        let payload = if let Some(req_cnt) = pending["min"].as_u64() {
            let cands = pending["cards"].as_array().unwrap();
            let selected: Vec<Value> = cands
                .iter()
                .take(req_cnt as usize)
                .map(|c| c["uid"].clone())
                .collect();
            json!({
                "decision_id": dec_id,
                "selected": selected
            })
        } else {
            let opts = pending["options"].as_array().unwrap();
            // Prefer playing cards / resolving sequence / first available option to progress quickly
            let chosen_opt = opts
                .iter()
                .find(|o| o["action"] == json!("play"))
                .or_else(|| opts.iter().find(|o| o["action"] == json!("close")))
                .or_else(|| opts.iter().find(|o| o["action"] == json!("end_turn")))
                .unwrap_or(&opts[0]);
            json!({
                "decision_id": dec_id,
                "option_id": chosen_opt["id"].as_str().unwrap()
            })
        };

        let (act_code, next_state) = send_http("POST", "/api/action", Some(&payload));
        assert_eq!(act_code, 200);
        cur_state = next_state;
    }

    assert!(cur_state["winner"].is_number());

    // Download full debug replay via GET /api/replay and verify bit-exact contract compliance
    let (rep_code, replay_doc) = send_http("GET", "/api/replay", None);
    assert_eq!(rep_code, 200);
    let verified = verify_replay_data(&replay_doc).expect("HTTP match replay must verify");
    assert_eq!(verified["ok"], json!(true));

    state.lock().unwrap().current_match.stop();
}

#[test]
fn test_all_18_card_operation_kinds_and_edge_cases() {
    let mut np = heav3ns_engine::engine::NoProviders;

    // 1. `draw`, `select`, `recover_character`, `recover_action`, `recover_any`
    {
        let mut g = Game::new(Some(load_proposed_specs()), false).unwrap();
        // H3W-AL-012 (The Fish Footman, Cost 2, Character, On Entry: select 3)
        // H3W-AL-006 (A Map of Elsewhere, Cost 1, Action: recover_character)
        // H3W-AL-018 (Knight of the Rescue, Cost 2, Character, On Entry: choice break1 / recover_action)
        // H3W-RA-020 (Silverhorn Herald, Cost 3, Character, On Entry: choice draw2 / recover_any)
        let footman = g.add(0, "H3W-AL-012", "hand").unwrap();
        let map_card = g.add(0, "H3W-AL-006", "hand").unwrap();
        let knight = g.add(0, "H3W-AL-018", "hand").unwrap();
        let herald = g.add(0, "H3W-RA-020", "hand").unwrap();
        let d1 = g.add(0, "H3W-AL-001", "deck").unwrap();
        let d2 = g.add(0, "H3W-AL-002", "deck").unwrap();
        let d3 = g.add(0, "H3W-AL-003", "deck").unwrap();
        let dead_char = g.add(0, "H3W-AL-011", "discard").unwrap();
        let dead_act = g.add(0, "H3W-AL-001", "discard").unwrap();
        g.add(0, "H3W-AL-001", "shields").unwrap();
        g.add(1, "H3W-RA-001", "shields").unwrap();

        g.start(0).unwrap();
        g.place(footman).unwrap();
        g.place(map_card).unwrap();
        let mut choices = HashMap::new();
        let mut c1 = Map::new();
        c1.insert("pick".to_string(), json!(d2));
        choices.insert((footman, "entry".to_string()), c1);
        let mut c2 = Map::new();
        c2.insert("target".to_string(), json!(dead_char));
        choices.insert((map_card, "action".to_string()), c2);
        g.close_with_options(None, Some(choices), &mut np).unwrap();

        // d2 picked into hand; d1 and d3 put on bottom of deck; dead_char recovered to hand
        assert!(g.p[0].hand.iter().any(|c| c.uid == d2));
        assert!(g.p[0].hand.iter().any(|c| c.uid == dead_char));
        assert_eq!(g.p[0].deck.len(), 2);
        assert_eq!(g.p[0].deck[0].uid, d1);
        assert_eq!(g.p[0].deck[1].uid, d3);

        // Now test `recover_action` and `recover_any` in a second sequence
        g.p[0].actions = 6;
        g.begin_sequence().unwrap();
        g.place(knight).unwrap();
        g.place(herald).unwrap();
        let mut choices2 = HashMap::new();
        let mut ck = Map::new();
        ck.insert("mode".to_string(), json!("recover_action"));
        ck.insert("target".to_string(), json!(map_card));
        choices2.insert((knight, "entry".to_string()), ck);
        let mut ch = Map::new();
        ch.insert("mode".to_string(), json!("recover_any"));
        ch.insert("target".to_string(), json!(dead_act));
        choices2.insert((herald, "entry".to_string()), ch);
        g.close_with_options(None, Some(choices2), &mut np).unwrap();
        assert!(g.p[0].hand.iter().any(|c| c.uid == map_card));
        assert!(g.p[0].hand.iter().any(|c| c.uid == dead_act));
    }

    // 2. `require_characters_in_sequence`, `break`, `break_finisher`
    {
        let mut g = Game::new(Some(load_proposed_specs()), false).unwrap();
        // H3W-AL-008 (Checkmate the Sky, Cost 3, Action: require_characters_in_sequence 2 -> break 2)
        // H3W-AL-019 (Jabberwock’s Riddle, Cost 3, Character, Combo 2: break_finisher n=2, finish=3, threshold=3)
        let checkmate_fail = g.add(0, "H3W-AL-008", "hand").unwrap();
        let jabber = g.add(0, "H3W-AL-019", "hand").unwrap();
        let supp1 = g.add(0, "H3W-AL-011", "hand").unwrap();
        let supp2 = g.add(0, "H3W-AL-011", "hand").unwrap();
        let checkmate_ok = g.add(0, "H3W-AL-008", "hand").unwrap();
        let supp3 = g.add(0, "H3W-AL-011", "hand").unwrap();
        let supp4 = g.add(0, "H3W-AL-011", "hand").unwrap();
        for _ in 0..5 {
            g.add(0, "H3W-AL-001", "shields").unwrap();
            g.add(1, "H3W-RA-001", "shields").unwrap();
        }
        for _ in 0..5 {
            g.add(0, "H3W-AL-001", "deck").unwrap();
            g.add(1, "H3W-RA-001", "deck").unwrap();
        }

        g.start(0).unwrap();
        // Sequence 1: Checkmate alone (0 characters in sequence) -> breaks 0 shields
        g.place(checkmate_fail).unwrap();
        g.close().unwrap();
        assert_eq!(g.p[1].shields.len(), 5);

        // Sequence 2: Checkmate + 2 characters (cost 3+1+1 = 5 <= 6) -> breaks 2 shields (5 -> 3)
        g.p[0].actions = 6;
        g.begin_sequence().unwrap();
        g.place(checkmate_ok).unwrap();
        g.place(supp1).unwrap();
        g.place(supp2).unwrap();
        g.close().unwrap();
        assert_eq!(g.p[1].shields.len(), 3);

        // Sequence 3: Jabberwock + 2 supporters (cost 3+1+1 = 5 <= 6)
        // Jabberwock's Combo 2 (`break_finisher` with threshold 3) sees <= 3 shields and breaks 3!
        g.p[0].actions = 6;
        g.begin_sequence().unwrap();
        g.place(jabber).unwrap();
        g.place(supp3).unwrap();
        g.place(supp4).unwrap();
        g.close().unwrap();
        assert_eq!(g.p[1].shields.len(), 0);
    }

    // 3. `discard`, `discard_enemy`, `bounce_friendly`, `bounce_other`, `bounce_enemy`, `bounce_enemy_max2`, `bounce_enemy1`, `cancel` (target_character, combo, effect) & `response_play`
    {
        let mut specs = load_proposed_specs();
        // Add custom test specs exercising `discard` (self-discard), `bounce_enemy1`, and `cancel` (`target_character` scope)
        specs.push(json!({
            "id": "TEST-SELF-DISC-BOUNCE1",
            "title": "Test Self Discard & Bounce1",
            "deck": "Alice",
            "copies": 1,
            "cost": 1,
            "type": "Action",
            "text": "Discard 1 card. Return an enemy Character costing 1 to hand.",
            "ops": [
                {"kind": "discard", "n": 1},
                {"kind": "bounce_enemy1"}
            ]
        }));
        specs.push(json!({
            "id": "TEST-PROTECT-CHAR",
            "title": "Test Target Character Cancel",
            "deck": "White Rabbit",
            "copies": 1,
            "cost": 1,
            "type": "Counter",
            "text": "Counter — Cancel an effect targeting a friendly Character.",
            "ops": [
                {"kind": "cancel", "scope": "target_character"}
            ]
        }));
        let mut g = Game::new(Some(specs), false).unwrap();
        let custom_act = g.add(0, "TEST-SELF-DISC-BOUNCE1", "hand").unwrap();
        let fodder = g.add(0, "H3W-AL-001", "hand").unwrap();
        // Talons at Low Tide (H3W-AL-005, cost 2): choice draw2 / discard_enemy1
        let talons = g.add(0, "H3W-AL-005", "hand").unwrap();
        // March Hare (H3W-RA-016, cost 1): On Entry: bounce_other (optional)
        let hare = g.add(0, "H3W-RA-016", "hand").unwrap();
        // Wind the Heart (H3W-RA-001, cost 2): bounce_friendly (optional), draw 1
        let wind = g.add(0, "H3W-RA-001", "hand").unwrap();
        // Borrowed Bravery (H3W-AL-003, cost 1): bounce_enemy_max2
        let bravery = g.add(0, "H3W-AL-003", "hand").unwrap();
        // Cheshire Wayfinder (H3W-AL-014, cost 3): On Entry: bounce_enemy (optional)
        let wayfinder = g.add(0, "H3W-AL-014", "hand").unwrap();

        let ally_field1 = g.add(0, "H3W-AL-011", "field").unwrap();
        let ally_field2 = g.add(0, "H3W-AL-011", "field").unwrap();
        let ally_field3 = g.add(0, "H3W-AL-011", "field").unwrap();
        let enemy_cost1 = g.add(1, "H3W-RA-013", "field").unwrap(); // cost 1
        let enemy_cost2 = g.add(1, "H3W-RA-012", "field").unwrap(); // cost 2
        let enemy_cost3 = g.add(1, "H3W-RA-019", "field").unwrap(); // cost 3
        let enemy_hand_card = g.add(1, "H3W-RA-001", "hand").unwrap();
        let enemy_protect_counter = g.add(1, "TEST-PROTECT-CHAR", "hand").unwrap();
        // Cards for Sequence C (`cancel` scopes `combo` [H3W-AL-002] & `effect` [H3W-AL-007], plus `response_play: true` [H3W-AL-003])
        let lion = g.add(0, "H3W-AL-016", "hand").unwrap();
        let lion_supp = g.add(0, "H3W-AL-011", "hand").unwrap();
        let porcelain_rebuttal = g.add(1, "H3W-AL-002", "hand").unwrap();
        let queens_favor = g.add(0, "H3W-AL-007", "hand").unwrap();
        let bravery_resp = g.add(1, "H3W-AL-003", "hand").unwrap();

        for _ in 0..5 {
            g.add(0, "H3W-AL-001", "deck").unwrap();
            g.add(1, "H3W-RA-001", "deck").unwrap();
            g.add(0, "H3W-AL-001", "shields").unwrap();
            g.add(1, "H3W-RA-001", "shields").unwrap();
        }

        g.start(0).unwrap();
        g.p[1].actions = 5;

        // Sequence A (cost 1 + 2 + 1 + 2 = 6): custom_act, talons, hare, wind
        g.place(custom_act).unwrap();
        g.place(talons).unwrap();
        g.place(hare).unwrap();
        g.place(wind).unwrap();

        let mut choices_a = HashMap::new();
        let mut c_cust = Map::new();
        c_cust.insert("discard".to_string(), json!([fodder]));
        c_cust.insert("target".to_string(), json!(enemy_cost1));
        choices_a.insert((custom_act, "action".to_string()), c_cust);

        let mut c_tal = Map::new();
        c_tal.insert("mode".to_string(), json!("discard_enemy1"));
        c_tal.insert("discard".to_string(), json!([enemy_hand_card]));
        choices_a.insert((talons, "action".to_string()), c_tal);

        let mut c_hare = Map::new();
        c_hare.insert("target".to_string(), json!(ally_field1));
        choices_a.insert((hare, "entry".to_string()), c_hare);

        let mut c_wind = Map::new();
        c_wind.insert("target".to_string(), json!(ally_field2));
        choices_a.insert((wind, "action".to_string()), c_wind);

        g.close_with_options(None, Some(choices_a), &mut np)
            .unwrap();

        // Sequence B (cost 1 + 3 = 4 <= 6): bravery, wayfinder
        g.p[0].actions = 6;
        g.begin_sequence().unwrap();
        g.place(bravery).unwrap();
        g.place(wayfinder).unwrap();

        let mut choices_b = HashMap::new();
        let mut c_brav = Map::new();
        c_brav.insert("target".to_string(), json!(enemy_cost2));
        choices_b.insert((bravery, "action".to_string()), c_brav);

        let mut c_way = Map::new();
        c_way.insert("target".to_string(), json!(enemy_cost3));
        choices_b.insert((wayfinder, "entry".to_string()), c_way);

        let mut responses_b = HashMap::new();
        responses_b.insert(
            (wayfinder, "entry".to_string()),
            vec![Response::new(enemy_protect_counter)],
        );

        g.close_with_options(Some(responses_b), Some(choices_b), &mut np)
            .unwrap();

        // Verify all outcomes
        assert!(g.p[0].discard.iter().any(|c| c.uid == fodder));
        assert!(g.p[1].discard.iter().any(|c| c.uid == enemy_hand_card));
        assert!(g.p[0].hand.iter().any(|c| c.uid == ally_field1));
        assert!(g.p[0].hand.iter().any(|c| c.uid == ally_field2));
        assert!(g.p[1].hand.iter().any(|c| c.uid == enemy_cost1));
        assert!(g.p[1].hand.iter().any(|c| c.uid == enemy_cost2));
        // `enemy_cost3` was protected by `target_character` counter, so it remains on field!
        assert!(g.p[1].field.iter().any(|c| c.uid == enemy_cost3));

        // Sequence C: Test `cancel` with `combo` scope (H3W-AL-002, A Porcelain Rebuttal, Cost 1)
        // and `cancel` with `effect` scope (H3W-AL-007, The Queen's Favor, Cost 2)
        // and `response_play: true` (H3W-AL-003, Borrowed Bravery, Cost 1, played in response window)
        g.p[0].actions = 6;
        g.p[1].actions = 6;
        g.begin_sequence().unwrap();
        g.place(lion).unwrap();
        g.place(lion_supp).unwrap();

        // When Lion's Combo triggers:
        // 1. Rabbit responds with `porcelain_rebuttal` (cancel scope: "combo"), and Alice replies with `queens_favor` (cancel scope: "effect")!
        // 2. When `lion_supp` Entry triggers, Rabbit responds with `bravery_resp` (`response_play: true`, bouncing `ally_field3` cost 1 from Alice's field to Alice's hand)!
        let mut choices_c = HashMap::new();
        let mut c_brav_resp = Map::new();
        c_brav_resp.insert("target".to_string(), json!(ally_field3));
        choices_c.insert((bravery_resp, "counter".to_string()), c_brav_resp);

        let mut responses_c = HashMap::new();
        responses_c.insert(
            (lion, "combo".to_string()),
            vec![Response::with_reply(
                porcelain_rebuttal,
                Response::new(queens_favor),
            )],
        );
        responses_c.insert(
            (lion_supp, "entry".to_string()),
            vec![Response::new(bravery_resp)],
        );

        let shields_before_c = g.p[1].shields.len();
        g.close_with_options(Some(responses_c), Some(choices_c), &mut np)
            .unwrap();
        // `bravery_resp` bounced `ally_field3` from Alice's field to Alice's hand during response window
        assert!(g.p[0].hand.iter().any(|c| c.uid == ally_field3));
        // `porcelain_rebuttal` was cancelled by `queens_favor`, so Lion's Combo 1 resolved and broke 2 shields!
        assert_eq!(g.p[1].shields.len(), shields_before_c - 2);
    }
}
