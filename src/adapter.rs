use crate::contract::snapshot;
use crate::engine::{
    as_strict_i32, is_bounce, load_proposed_specs, Card, DecisionProviders, Effect, Game, Illegal,
    Response,
};
use crate::policy::{card_value, AuditPolicy};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Condvar, Mutex,
};
use std::thread;

static GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_generation_hex() -> String {
    let n = GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(n.to_le_bytes());
    hasher.update(nanos.to_le_bytes());
    let digest = format!("{:x}", hasher.finalize());
    digest[..32].to_string()
}

pub fn pool_sha256() -> String {
    let raw = include_bytes!("../data/proposed.json");
    let mut hasher = Sha256::new();
    hasher.update(raw);
    format!("{:x}", hasher.finalize())
}

pub fn runtime_hashes() -> Value {
    json!({
        "engine.py": "9a01d7402d8c850967d4869061c9b879d5b122e69580784f86e2d146145b154d",
        "simulate_games.py": "100336330e91cbf37376a58dd68c02117108de6bf1f9a732d828b90897598843",
        "audit_policy.py": "118ca4f1604510e7880e17cdde49bc477339fd6bfdccffd45f316fe05e6a6c3d",
        "transition_support.py": "76f16011b97b3994f5f6fbbf8b8edb3a67be9fdd8ad70fec805230996efbd935",
        "r2_contract.py": "4b7e1529d1afc6828c5b6be829a6bbf92fbfd93826624739f77c3812b53ec88a",
        "replay_transitions.py": "ae3aa64e99fcf6c7926563ff1e89dd944d5bffee7599414c1bd637bae6f03802"
    })
}

pub fn mode_label(mode: Option<&str>) -> String {
    match mode {
        Some("bounce_friendly") => "Return a friendly Character to hand".to_string(),
        Some("break1") => "Break 1 Shield".to_string(),
        Some("break2") => "Break 2 Shields".to_string(),
        Some("discard_enemy") | Some("discard_enemy1") => "Opponent discards 1 card".to_string(),
        Some("discard_enemy2") => "Opponent discards 2 cards".to_string(),
        Some("draw1") => "Draw 1 card".to_string(),
        Some("draw2") => "Draw 2 cards".to_string(),
        Some("recover_action") => "Recover an Action or Counter from discard".to_string(),
        Some("recover_any") => "Recover a card from discard".to_string(),
        Some("recover_character") => "Recover a Character from discard".to_string(),
        Some("skip") => "Skip the optional return".to_string(),
        Some(other) if !other.is_empty() => other.replace('_', " "),
        _ => "Resolve effect".to_string(),
    }
}

/// Builds the exact seeded starting layout matching `adapter.stock_game(seed)`.
pub fn stock_game(seed: u64) -> Result<Game, String> {
    if seed > 2_147_483_647 {
        return Err("Seed must be an integer from 0 to 2147483647".to_string());
    }
    let specs = load_proposed_specs();
    let mut game = Game::new_sim(Some(specs.clone()), false).map_err(|e| e.0)?;
    for (owner, deck_name) in ["Alice", "White Rabbit"].iter().enumerate() {
        let mut ids = Vec::new();
        // Crucial: iterate in the order cards appear in `proposed.json` (`specs`), matching Python's `game.specs.values()` insertion order!
        for c in &specs {
            if c.get("deck").and_then(|v| v.as_str()) == Some(*deck_name) {
                let copies = c.get("copies").and_then(as_strict_i32).unwrap_or(0) as usize;
                let id = c.get("id").and_then(|v| v.as_str()).unwrap().to_string();
                for _ in 0..copies {
                    ids.push(id.clone());
                }
            }
        }
        if ids.len() != 40 {
            return Err("Invalid stock deck".to_string());
        }
        game.rng.seed_u64(seed * 2 + (owner as u64));
        game.rng.shuffle(&mut ids);
        for (i, cid) in ids.iter().enumerate() {
            let zone = if i < 6 {
                "shields"
            } else if i < 11 {
                "hand"
            } else {
                "deck"
            };
            game.add(owner, cid, zone).map_err(|e| e.0)?;
        }
    }
    Ok(game)
}

/// Builds a 30-card player deck and a matching 30-card opponent for the Unity deck studio.
/// The existing 40-card stock format remains unchanged for replays and other clients.
pub fn custom_game(seed: u64, player_ids: &[String]) -> Result<(Game, [String; 2]), String> {
    custom_game_for_stage(seed, player_ids, 0)
}

pub fn custom_game_for_stage(seed: u64, player_ids: &[String], stage: u32) -> Result<(Game, [String; 2]), String> {
    if seed > 2_147_483_647 || player_ids.len() != 30 || stage > 3 {
        return Err("A custom deck must contain exactly 30 cards and use a valid seed".into());
    }
    let specs = load_proposed_specs();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut faction: Option<&str> = None;
    for id in player_ids {
        let card = specs.iter().find(|c| c.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .ok_or_else(|| format!("Unknown card {id}"))?;
        let deck = card.get("deck").and_then(Value::as_str).ok_or("Missing faction")?;
        if !matches!(deck, "Alice" | "White Rabbit") || faction.is_some_and(|f| f != deck) {
            return Err("Deck must contain cards from one faction".into());
        }
        faction = Some(deck);
        let count = counts.entry(id).or_insert(0);
        *count += 1;
        let limit = card.get("copies").and_then(as_strict_i32).unwrap_or(0).max(0) as usize;
        if *count > limit { return Err(format!("Too many copies of {id}")); }
    }
    let player = faction.ok_or("Empty deck")?;
    let opponent = if player == "Alice" { "White Rabbit" } else { "Alice" };
    let mut rival = Vec::new();
    let mut rival_specs: Vec<&Value> = specs.iter().filter(|card| card.get("deck").and_then(Value::as_str) == Some(opponent)).collect();
    if stage == 2 { rival_specs.reverse(); }
    if stage == 3 { rival_specs.sort_by_key(|card| std::cmp::Reverse(card.get("cost").and_then(Value::as_i64).unwrap_or(0))); }
    for card in rival_specs {
        let id = card.get("id").and_then(Value::as_str).ok_or("Missing card id")?;
        let copies = card.get("copies").and_then(as_strict_i32).unwrap_or(0).max(0);
        for _ in 0..copies { if rival.len() < 30 { rival.push(id.to_string()); } }
    }
    if rival.len() != 30 { return Err("Opponent deck is incomplete".into()); }
    let names = [player.to_string(), opponent.to_string()];
    let mut game = Game::new_sim(Some(specs), false).map_err(|e| e.0)?;
    for (owner, mut ids) in [player_ids.to_vec(), rival].into_iter().enumerate() {
        game.rng.seed_u64(seed * 2 + owner as u64);
        game.rng.shuffle(&mut ids);
        for (i, id) in ids.iter().enumerate() {
            let zone = if i < 6 { "shields" } else if i < 11 { "hand" } else { "deck" };
            game.add(owner, id, zone).map_err(|e| e.0)?;
        }
    }
    Ok((game, names))
}

pub fn serialize_card(c: &Card) -> Value {
    json!({
        "uid": c.uid,
        "id": c.id(),
        "name": c.title(),
        "type": c.card_type(),
        "cost": c.cost(),
        "text": c.text(),
        "combo_n": c.combo_n(),
        "art_url": format!("/assets/cards/{}.webp", c.id()),
        "owner": c.owner
    })
}

pub fn build_history(g: &Game, limit: Option<usize>) -> Vec<Value> {
    let mut out = Vec::new();
    for event in &g.log {
        let k = event.first().and_then(|v| v.as_str()).unwrap_or("");
        let mut text: Option<String> = None;
        match k {
            "start" => {
                let seat = event.get(1).and_then(as_strict_i32).unwrap_or(0);
                let who = if seat == 0 { "Alice" } else { "White Rabbit" };
                text = Some(format!("{who} begins a turn."));
            }
            "place" | "sequence_discard" | "return" | "recover" | "discard" | "end_discard"
            | "shield_discard" => {
                if let Some(uid) = event.get(1).and_then(as_strict_i32) {
                    if let Some(c) = g.cards.get(&uid) {
                        let verb = match k {
                            "place" => "played",
                            "sequence_discard" => "moved to discard",
                            "return" => "returned to hand from the field",
                            "recover" => "recovered from discard to hand",
                            "discard" => "discarded from hand by an effect",
                            "end_discard" => "discarded from hand to keep six",
                            "shield_discard" => "revealed Shield moved to discard",
                            _ => "",
                        };
                        text = Some(format!("{}: {verb}.", c.title()));
                    }
                }
            }
            "draw" => {
                let seat = event.get(1).and_then(as_strict_i32).unwrap_or(0);
                let who = if seat == 0 { "Alice" } else { "White Rabbit" };
                text = Some(format!("{who} drew a card."));
            }
            "select" => {
                text = Some("A selection effect put a card into hand.".to_string());
            }
            "window" => {
                if let (Some(uid), Some(kind_str)) = (
                    event.get(1).and_then(as_strict_i32),
                    event.get(2).and_then(|v| v.as_str()),
                ) {
                    if let Some(c) = g.cards.get(&uid) {
                        text = Some(format!("{}: {kind_str} response window.", c.title()));
                    }
                }
            }
            "counter" => {
                if let (Some(u1), Some(u2)) = (
                    event.get(1).and_then(as_strict_i32),
                    event.get(2).and_then(as_strict_i32),
                ) {
                    if let (Some(c1), Some(c2)) = (g.cards.get(&u1), g.cards.get(&u2)) {
                        text = Some(format!("{} responds to {}.", c1.title(), c2.title()));
                    }
                }
            }
            "cancel" => {
                if let (Some(uid), Some(kind_str)) = (
                    event.get(1).and_then(as_strict_i32),
                    event.get(2).and_then(|v| v.as_str()),
                ) {
                    if let Some(c) = g.cards.get(&uid) {
                        text = Some(format!("{}: {kind_str} cancelled.", c.title()));
                    }
                }
            }
            "resolve" => {
                if let (Some(uid), Some(kind_str)) = (
                    event.get(1).and_then(as_strict_i32),
                    event.get(2).and_then(|v| v.as_str()),
                ) {
                    if let Some(c) = g.cards.get(&uid) {
                        text = Some(format!("{}: {kind_str} started resolving.", c.title()));
                    }
                }
            }
            "combo_lost_support" => {
                if let Some(uid) = event.get(1).and_then(as_strict_i32) {
                    if let Some(c) = g.cards.get(&uid) {
                        text = Some(format!("{}: Combo lost its later support.", c.title()));
                    }
                }
            }
            "mode_declared" => {
                if let Some(uid) = event.get(1).and_then(as_strict_i32) {
                    let m = event.get(5).and_then(|v| v.as_str());
                    if let Some(c) = g.cards.get(&uid) {
                        text = Some(format!("{}: chose {}.", c.title(), mode_label(m)));
                    }
                }
            }
            "reveal" => {
                if let Some(uid) = event.get(1).and_then(as_strict_i32) {
                    if let Some(c) = g.cards.get(&uid) {
                        let mut s = format!("Shield revealed: {}.", c.title());
                        if let Some(st) = c.shield_trigger() {
                            s.push_str(" Shield Trigger: ");
                            s.push_str(st);
                        }
                        text = Some(s);
                    }
                }
            }
            "win" => {
                let seat = event.get(1).and_then(as_strict_i32).unwrap_or(0);
                let who = if seat == 0 { "Alice" } else { "White Rabbit" };
                text = Some(format!("{who} wins."));
            }
            _ => {}
        }
        if let Some(t) = text {
            let id = out.len();
            out.push(json!({
                "id": id,
                "text": t,
                "event": k
            }));
        }
    }
    if let Some(lim) = limit {
        let start = out.len().saturating_sub(lim);
        out[start..].to_vec()
    } else {
        out
    }
}

pub fn build_latest_reveal(g: &Game, generation: &str) -> Value {
    let mut latest = Value::Null;
    let mut turn = 0;
    for (index, event) in g.log.iter().enumerate() {
        let k = event.first().and_then(|v| v.as_str()).unwrap_or("");
        if k == "start" {
            turn += 1;
        } else if k == "reveal" {
            if let Some(uid) = event.get(1).and_then(as_strict_i32) {
                if let Some(c) = g.cards.get(&uid) {
                    latest = json!({
                        "id": format!("{generation}:reveal:{index}"),
                        "card": serialize_card(c),
                        "trigger": c.shield_trigger(),
                        "owner": c.owner,
                        "turn": turn
                    });
                }
            }
        }
    }
    latest
}

pub fn build_sequence_details(g: &Game, generation: &str) -> Vec<Value> {
    let mut rows = Vec::new();
    for seq_c in &g.sequence {
        let c = g.cards.get(&seq_c.uid).unwrap_or(seq_c);
        let co_n = c.combo_n();
        let placement_index = g.log.iter().enumerate().rev().find_map(|(i, ev)| {
            if ev.first().and_then(|v| v.as_str()) == Some("place")
                && ev.get(1).and_then(as_strict_i32) == Some(c.uid)
            {
                Some(i)
            } else {
                None
            }
        });
        let mut effect_events = Vec::new();
        if let Some(p_idx) = placement_index {
            for (index, event) in g.log.iter().enumerate().skip(p_idx + 1) {
                let ek = event.first().and_then(|v| v.as_str()).unwrap_or("");
                if matches!(ek, "window" | "resolve" | "cancel" | "combo_lost_support")
                    && event.get(1).and_then(as_strict_i32) == Some(c.uid)
                {
                    let kind_str = event.get(2).and_then(|v| v.as_str()).unwrap_or("combo");
                    effect_events.push(json!({
                        "id": format!("{generation}:event:{index}"),
                        "event": ek,
                        "kind": kind_str
                    }));
                }
            }
        }
        let supporters = if co_n.is_some() {
            g.supporters(c)
        } else {
            Vec::new()
        };
        let entry_present = if c.card_type() == "Character" {
            g.entry_ops(c).map(|v| !v.is_empty()).unwrap_or(false)
        } else {
            c.spec
                .get("ops")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        };
        let p_ev_id = placement_index
            .map(|idx| json!(format!("{generation}:event:{idx}")))
            .unwrap_or(Value::Null);
        rows.push(json!({
            "card": serialize_card(c),
            "entry_present": entry_present,
            "combo_n": co_n,
            "support_count": supporters.len(),
            "support_uids": supporters.iter().map(|x| x.uid).collect::<Vec<i32>>(),
            "combo_eligible": g.combo_eligible(c),
            "combo_used": c.combo_used,
            "present": g.present(c.uid),
            "placement_event_id": p_ev_id,
            "effect_events": effect_events
        }));
    }
    rows
}

pub fn build_timeline(g: &Game, generation: &str) -> Vec<Value> {
    let mut rows = Vec::new();
    for (index, event) in g.log.iter().enumerate() {
        let kind = event.first().and_then(|v| v.as_str()).unwrap_or("");
        let label = match kind {
            "place" => "Played",
            "window" => "Response window opened",
            "counter" => "Response played",
            "resolve" => "Effect started resolving",
            "cancel" => "Effect cancelled",
            "combo_lost_support" => "Combo lost later support",
            "sequence_discard" => "Sequence cleanup: moved to discard",
            "return" => "Returned from field to hand",
            "recover" => "Recovered from discard to hand",
            "discard" => "Effect: discarded from hand",
            "end_discard" => "Hand limit: discarded to keep six",
            "shield_discard" => "Revealed Shield moved to discard",
            "reveal" => "Shield revealed",
            "start" => "Turn started",
            "win" => "Match won",
            _ => continue,
        };
        let source = if kind != "start" && kind != "win" {
            event
                .get(1)
                .and_then(as_strict_i32)
                .and_then(|u| g.cards.get(&u))
        } else {
            None
        };
        let ev_kind = if matches!(kind, "window" | "resolve" | "cancel") {
            event.get(2).cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        let owner_val = if let Some(s) = source {
            json!(s.owner)
        } else {
            event.get(1).cloned().unwrap_or(Value::Null)
        };
        let mut row = Map::new();
        row.insert(
            "id".to_string(),
            json!(format!("{generation}:event:{index}")),
        );
        row.insert("event".to_string(), json!(kind));
        row.insert("label".to_string(), json!(label));
        row.insert(
            "source".to_string(),
            source.map(serialize_card).unwrap_or(Value::Null),
        );
        row.insert("kind".to_string(), ev_kind);
        row.insert("owner".to_string(), owner_val);
        row.insert("status".to_string(), json!("recorded"));
        row.insert("historical".to_string(), json!(true));

        if kind == "counter" {
            if let Some(s) = source {
                let ck = match s.card_type() {
                    "Character" => "entry",
                    "Action" => "action",
                    _ => "counter",
                };
                row.insert(
                    "parent_uid".to_string(),
                    event.get(2).cloned().unwrap_or(Value::Null),
                );
                row.insert("kind".to_string(), json!(ck));
                row.insert(
                    "target_kind".to_string(),
                    event.get(3).cloned().unwrap_or(Value::Null),
                );
                row.insert("is_response".to_string(), json!(true));
            }
        }
        if kind == "return" || kind == "recover" {
            row.insert("destination".to_string(), json!("hand"));
        } else if matches!(
            kind,
            "sequence_discard" | "discard" | "end_discard" | "shield_discard"
        ) {
            row.insert("destination".to_string(), json!("discard"));
        }
        if kind == "cancel" {
            let k_str = event.get(2).and_then(|v| v.as_str()).unwrap_or("effect");
            row.insert(
                "detail".to_string(),
                json!(format!(
                    "Only this {k_str} effect is cancelled. Other effects and card destinations are handled separately."
                )),
            );
        } else if kind == "combo_lost_support" {
            row.insert(
                "detail".to_string(),
                json!(
                    "The pending Combo did not resolve because too few later supporters remained on the field."
                ),
            );
        } else if kind == "return" {
            row.insert(
                "detail".to_string(),
                json!(
                    "Returned to its owner’s hand, not discarded. A returned sequence card no longer supplies forward Combo support."
                ),
            );
        } else if kind == "recover" {
            row.insert(
                "detail".to_string(),
                json!("Moved from its owner’s discard into their hand."),
            );
        }
        if let Some(s) = source {
            let cz = ["hand", "field", "discard", "shields", "deck"]
                .into_iter()
                .find(|&z| g.p[s.owner].zone(z).iter().any(|x| x.uid == s.uid));
            row.insert(
                "current_zone".to_string(),
                cz.map(|z| json!(z)).unwrap_or(Value::Null),
            );
        }
        rows.push(Value::Object(row));
    }
    let start = rows.len().saturating_sub(24);
    rows[start..].to_vec()
}

pub fn build_hand_availability(g: &Game, pending: Option<&Value>) -> Vec<Value> {
    let options = pending
        .and_then(|p| p.get("options"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut rows = Vec::new();
    for c in &g.p[0].hand {
        let actions: Vec<String> = options
            .iter()
            .filter(|o| {
                o.get("card_uid").and_then(as_strict_i32) == Some(c.uid)
                    && matches!(
                        o.get("action").and_then(|v| v.as_str()),
                        Some("play") | Some("respond")
                    )
            })
            .filter_map(|o| o.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
        let p_kind = pending.and_then(|p| p.get("kind")).and_then(|v| v.as_str());
        let is_reply = pending
            .and_then(|p| p.get("effect"))
            .and_then(|e| e.get("is_reply"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let reason = if !actions.is_empty() {
            "Available: choose this card, then confirm.".to_string()
        } else if pending.is_none() {
            "Wait for the current effect or opponent decision.".to_string()
        } else if p_kind == Some("response") {
            if c.cost() > g.p[0].actions {
                format!("Needs {} Actions; you have {}.", c.cost(), g.p[0].actions)
            } else if is_reply && c.card_type() != "Counter" {
                "Only Counters can answer a response.".to_string()
            } else {
                "This card cannot answer this effect with a legal target.".to_string()
            }
        } else if p_kind != Some("turn") {
            "Finish the current decision before playing another card.".to_string()
        } else if c.card_type() == "Counter" {
            "Counters are played in an eligible response window.".to_string()
        } else if g.phase != "placement" {
            "Begin another sequence before playing this card.".to_string()
        } else if c.cost() > g.p[0].actions {
            format!("Needs {} Actions; you have {}.", c.cost(), g.p[0].actions)
        } else {
            "Not available in the current decision.".to_string()
        };
        rows.push(json!({
            "uid": c.uid,
            "playable": !actions.is_empty(),
            "reason": reason,
            "actions": actions,
            "cost": c.cost()
        }));
    }
    rows
}

#[derive(Clone, Debug)]
pub enum OptionPayload {
    TurnAction(String, Option<i32>),
    ResponseUid(Option<i32>),
    ChoiceMap(Map<String, Value>),
    PickUid(i32),
}

#[derive(Clone, Debug)]
pub enum AnswerValue {
    Option(OptionPayload),
    SelectedCards(Vec<i32>),
}

pub struct PendingDecision {
    pub public: Value,
    pub values: HashMap<String, OptionPayload>,
    pub cards: Vec<i32>,
    pub count: Option<usize>,
}

pub struct SharedMatchState {
    pub stopped: bool,
    pub answer: Option<AnswerValue>,
    pub pending: Option<PendingDecision>,
    pub revision: u64,
    pub decision_count: u64,
    pub inputs: Vec<Value>,
    pub error: Option<String>,
    pub internal_error: Option<String>,
    pub cached: Value,
    pub transitions: Vec<Value>,
    pub log: Vec<Vec<Value>>,
    pub winner: Option<usize>,
}

/// Interactive Human-vs-AI match coordinator matching `adapter.Match`.
pub struct Match {
    pub seed: u64,
    pub starter: usize,
    pub generation: String,
    pub initial: Value,
    pub shared: Arc<(Mutex<SharedMatchState>, Condvar)>,
}

pub fn choices_for(
    g: &Game,
    c: &Card,
    kind: &str,
    ops: &[Value],
    pending: Option<&Effect>,
) -> Vec<(Value, Map<String, Value>)> {
    let modes: Vec<String> = ops
        .iter()
        .filter(|o| o.get("kind").and_then(|v| v.as_str()) == Some("choice"))
        .filter_map(|o| o.get("choices").and_then(|v| v.as_array()))
        .flatten()
        .filter_map(|m| m.as_str().map(|s| s.to_string()))
        .collect();
    let choices: Vec<Map<String, Value>> = if !modes.is_empty() {
        modes
            .iter()
            .map(|m| {
                let mut map = Map::new();
                map.insert("mode".to_string(), json!(m));
                map
            })
            .collect()
    } else {
        vec![Map::new()]
    };
    let mut result = Vec::new();
    for choice in choices {
        let expanded = g.expanded(ops, &choice).unwrap_or_default();
        let e = Effect::new(
            c.clone(),
            kind,
            expanded.clone(),
            choice.clone(),
            pending.map(|p| Box::new(p.clone())),
        );
        let mut target_lists: Vec<Vec<Card>> = Vec::new();
        for op in &expanded {
            let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if is_bounce(k) || k == "counter_bounce_later" {
                target_lists.push(g.bounce_candidates(&e, k));
            } else if k.starts_with("recover") {
                let cands: Vec<Card> = g.p[c.owner]
                    .discard
                    .iter()
                    .filter(|t| match k {
                        "recover_character" => t.card_type() == "Character",
                        "recover_action" => t.card_type() == "Action" || t.card_type() == "Counter",
                        _ => true,
                    })
                    .cloned()
                    .collect();
                target_lists.push(cands);
            }
        }
        let targets = target_lists.first();
        let mut variants: Vec<Map<String, Value>> = match targets {
            Some(t_list) if !t_list.is_empty() => t_list
                .iter()
                .map(|t| {
                    let mut m = choice.clone();
                    m.insert("target".to_string(), json!(t.uid));
                    m
                })
                .collect(),
            _ => vec![choice.clone()],
        };
        let has_optional = expanded
            .iter()
            .any(|o| o.get("optional").and_then(|v| v.as_bool()).unwrap_or(false));
        if pending.is_none() && has_optional && modes.is_empty() {
            let mut skip_m = Map::new();
            skip_m.insert("mode".to_string(), json!("skip"));
            variants.push(skip_m);
        }
        for value in variants {
            let mut test = Effect::new(
                c.clone(),
                kind,
                expanded.clone(),
                value.clone(),
                pending.map(|p| Box::new(p.clone())),
            );
            g.prepare(&mut test);
            if let Some(p) = pending {
                if !g.counter_eligible(c, p, &expanded) {
                    continue;
                }
                let has_cbl = expanded.iter().any(|o| {
                    o.get("kind").and_then(|v| v.as_str()) == Some("counter_bounce_later")
                });
                if (c.response_play() || has_cbl)
                    && (test
                        .choice
                        .get("target")
                        .map(|v| v.is_null())
                        .unwrap_or(true)
                        || value.get("mode").and_then(|v| v.as_str()) == Some("skip"))
                {
                    continue;
                }
            }
            let mode_str = value.get("mode").and_then(|v| v.as_str());
            let mut label = mode_label(mode_str);
            let target_card = value
                .get("target")
                .and_then(as_strict_i32)
                .and_then(|u| g.cards.get(&u));
            if let Some(tc) = target_card {
                label.push_str(" → ");
                label.push_str(tc.title());
            }
            if matches!(targets, Some(t_list) if t_list.is_empty()) {
                label.push_str(" (no eligible target; instruction has no effect)");
            }
            let opt_json = json!({
                "label": label,
                "mode": value.get("mode").cloned().unwrap_or(Value::Null),
                "target_uid": value.get("target").cloned().unwrap_or(Value::Null),
                "card": target_card.map(serialize_card).unwrap_or(Value::Null),
            });
            result.push((opt_json, value));
        }
    }
    result
}

pub fn publish_state(
    shared: &Arc<(Mutex<SharedMatchState>, Condvar)>,
    g: &Game,
    seed: u64,
    generation: &str,
    status: &str,
    pending_public: Option<Value>,
) {
    let public_history = build_history(g, None);
    let mut players = Vec::new();
    for (owner, p) in g.p.iter().enumerate() {
        let hand_json: Vec<Value> = if owner == 0 {
            p.hand.iter().map(serialize_card).collect()
        } else {
            p.hand.iter().map(|_| json!({"hidden": true})).collect()
        };
        let shields_json: Vec<Value> = p.shields.iter().map(|_| json!({"hidden": true})).collect();
        let field_json: Vec<Value> = p.field.iter().map(serialize_card).collect();
        let discard_json: Vec<Value> = p.discard.iter().map(serialize_card).collect();
        players.push(json!({
            "name": if owner == 0 { "Alice" } else { "White Rabbit" },
            "actions": p.actions,
            "hand": hand_json,
            "hand_count": p.hand.len(),
            "deck_count": p.deck.len(),
            "shields": shields_json,
            "field": field_json,
            "discard": discard_json
        }));
    }
    let seq_uids: Vec<i32> = g
        .sequence
        .iter()
        .filter(|c| g.present(c.uid))
        .map(|c| c.uid)
        .collect();
    let timeline = build_timeline(g, generation);
    let sequence_details = build_sequence_details(g, generation);
    let hand_availability = build_hand_availability(g, pending_public.as_ref());
    let hist_total = public_history.len();
    let hist_slice_start = hist_total.saturating_sub(300);
    let hist_slice = public_history[hist_slice_start..].to_vec();
    let latest_reveal = build_latest_reveal(g, generation);

    let (lock, cvar) = &**shared;
    let mut st = lock.lock().unwrap();
    st.revision += 1;
    st.transitions = g.transitions.clone();
    st.log = g.log.clone();
    st.winner = g.winner;
    let err_val = st.error.as_ref().map(|s| json!(s)).unwrap_or(Value::Null);
    let pending_val = pending_public.unwrap_or(Value::Null);
    st.cached = json!({
        "revision": st.revision,
        "generation": generation,
        "status": status,
        "seed": seed,
        "active": g.active,
        "turn": g.turn_number,
        "phase": g.phase,
        "winner": g.winner,
        "players": players,
        "sequence": seq_uids,
        "pending": pending_val,
        "timeline": timeline,
        "sequence_details": sequence_details,
        "hand_availability": hand_availability,
        "history": hist_slice,
        "history_total": hist_total,
        "history_limit": 300,
        "history_truncated": hist_total > 300,
        "latest_reveal": latest_reveal,
        "error": err_val,
        "replay_available": status == "finished" || status == "error"
    });
    cvar.notify_all();
}

#[allow(clippy::too_many_arguments)]
fn ask_human(
    shared: &Arc<(Mutex<SharedMatchState>, Condvar)>,
    g: &Game,
    seed: u64,
    generation: &str,
    kind: &str,
    title: &str,
    options: Vec<(Map<String, Value>, OptionPayload)>,
    source: Option<&Card>,
    count: Option<usize>,
    cards: Option<&[Card]>,
    effect: Option<Value>,
    payment_due: i32,
    discard_reason: Option<&str>,
) -> Result<AnswerValue, Illegal> {
    let (lock, cvar) = &**shared;
    let decision_id = {
        let mut st = lock.lock().unwrap();
        if st.stopped {
            return Err(Illegal("STOPPED".into()));
        }
        st.decision_count += 1;
        format!("{generation}:{}", st.decision_count)
    };

    let mut d = Map::new();
    d.insert("id".to_string(), json!(decision_id));
    d.insert("kind".to_string(), json!(kind));
    d.insert("title".to_string(), json!(title));

    let (mut happened, mut after, mut instruction) = match kind {
        "turn" => (
            "Your turn is ready for a decision.".to_string(),
            "The selected card is paid for and placed, or the chosen sequence/turn command runs."
                .to_string(),
            "Place cards left to right, then resolve the sequence. Saved Actions can pay for later responses."
                .to_string(),
        ),
        "response" => (
            "An effect is waiting for your response.".to_string(),
            "A response card selection is committed. If it needs a mode or target, that decision follows before its shown cost is paid; then Counter replies may cancel it. Pass lets priority continue."
                .to_string(),
            "Review the announced effect and locked target before choosing.".to_string(),
        ),
        "choice" => (
            "An already-played effect needs a choice.".to_string(),
            "Your mode and target are locked; the engine continues the effect and its response window."
                .to_string(),
            "Choose one listed mode or target. This does not pay the card cost again."
                .to_string(),
        ),
        "pick" => (
            "Your effect revealed private cards for selection.".to_string(),
            "The selected card enters your hand; the engine handles the remaining cards."
                .to_string(),
            "Choose one of the revealed cards.".to_string(),
        ),
        "discard" => (
            "A discard instruction needs your selection.".to_string(),
            "Exactly the selected cards move from your hand to discard.".to_string(),
            "Select the required number of cards before confirming.".to_string(),
        ),
        _ => (String::new(), String::new(), String::new()),
    };

    if kind == "discard" {
        let r = discard_reason.unwrap_or("effect");
        d.insert("reason".to_string(), json!(r));
        let cnt = count.unwrap_or(0);
        if r == "hand_limit" {
            happened = "Your turn is ending and your hand exceeds the six-card limit.".to_string();
            after = "The selected cards move to discard, leaving six cards in your hand; then the next turn begins.".to_string();
            instruction = format!(
                "Select exactly {cnt} {} to discard, keeping six.",
                if cnt == 1 { "card" } else { "cards" }
            );
        } else {
            happened = "A card effect requires you to discard cards from your hand.".to_string();
            after = "The selected cards move to discard, then the current effect continues."
                .to_string();
            instruction = format!(
                "Select exactly {cnt} cards. This is an effect instruction, not an end-turn hand-limit check."
            );
        }
    }
    if kind == "choice" && payment_due > 0 {
        happened =
            "You selected a response card; choose its mode or target before its play cost is paid."
                .to_string();
        after = format!(
            "This locks the choice and pays {payment_due} Actions for the response card. Counter replies may still cancel its effect."
        );
        instruction = "The response card selection is committed. You can change this local mode or target selection before confirming, but cannot rewind the previous submitted decision.".to_string();
    } else if kind == "choice" {
        after = "This locks the mode or target. A response window may still cancel the effect before it resolves; no additional Actions are paid.".to_string();
    }

    let mut payloads = HashMap::new();
    let mut public_options = Vec::new();
    for (i, (mut item, payload)) in options.into_iter().enumerate() {
        let oid = i.to_string();
        item.insert("id".to_string(), json!(oid));
        let action_str = item
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cost = if action_str == "play" || action_str == "respond" {
            item.get("card_uid")
                .and_then(as_strict_i32)
                .and_then(|u| g.cards.get(&u))
                .map(|c| c.cost())
                .unwrap_or(payment_due)
        } else {
            payment_due
        };
        item.insert("cost".to_string(), json!(cost));
        let opt_after = match action_str.as_str() {
            "play" => format!(
                "Pay {cost} Actions and place this card in the sequence. Its effects wait until you resolve that sequence."
            ),
            "respond" => format!(
                "Commit this response-card selection. If a mode or target is needed, choose it next; the card costs {cost} Actions once. Counter replies may still cancel its effect."
            ),
            "pass" => "Pass this response window without spending Actions. Priority continues for the shown effect; it may resolve after the required passes.".to_string(),
            "begin_sequence" => "Open another empty sequence using your remaining Actions.".to_string(),
            "close_sequence" => "Close this sequence. The engine processes cards in played order, pausing for choices and response windows. You cannot add more cards to this closed sequence.".to_string(),
            "end_turn" => "End your turn. If your hand exceeds six cards, choose discards next. Unused Actions remain available for responses.".to_string(),
            _ => after.clone(),
        };
        item.insert("after_confirm".to_string(), json!(opt_after));
        payloads.insert(oid, payload);
        public_options.push(Value::Object(item));
    }
    d.insert("options".to_string(), Value::Array(public_options));

    if let Some(src) = source {
        d.insert("source".to_string(), serialize_card(src));
    }
    if let Some(eff) = effect {
        d.insert("effect".to_string(), eff);
    }
    d.insert("payment_due".to_string(), json!(payment_due));
    d.insert(
        "source_cost".to_string(),
        source.map(|s| json!(s.cost())).unwrap_or(Value::Null),
    );
    d.insert(
        "context".to_string(),
        json!({
            "what_happened": happened,
            "responder": "Alice",
            "after_confirm": after,
            "instruction": instruction
        }),
    );
    d.insert(
        "commit_notice".to_string(),
        json!("You can cancel a local selection before confirming. Confirmed engine actions cannot be undone."),
    );

    let card_uids: Vec<i32> = cards
        .map(|cs| cs.iter().map(|c| c.uid).collect())
        .unwrap_or_default();
    if let (Some(cnt), Some(cs)) = (count, cards) {
        d.insert("min".to_string(), json!(cnt));
        d.insert("max".to_string(), json!(cnt));
        d.insert(
            "cards".to_string(),
            Value::Array(cs.iter().map(serialize_card).collect()),
        );
    }

    let public_d = Value::Object(d);
    {
        let mut st = lock.lock().unwrap();
        st.pending = Some(PendingDecision {
            public: public_d.clone(),
            values: payloads,
            cards: card_uids,
            count,
        });
        st.answer = None;
    }
    publish_state(shared, g, seed, generation, "waiting", Some(public_d));

    let ans = {
        let mut st = lock.lock().unwrap();
        while st.answer.is_none() && !st.stopped {
            st = cvar.wait(st).unwrap();
        }
        if st.stopped {
            return Err(Illegal("STOPPED".into()));
        }
        let res = st.answer.take().unwrap();
        st.pending = None;
        res
    };
    publish_state(shared, g, seed, generation, "thinking", None);
    Ok(ans)
}

struct MatchProviders {
    shared: Arc<(Mutex<SharedMatchState>, Condvar)>,
    seed: u64,
    generation: String,
    ai: AuditPolicy,
}

impl DecisionProviders for MatchProviders {
    fn has_choice(&self) -> bool {
        true
    }
    fn has_response(&self) -> bool {
        true
    }
    fn has_pick(&self) -> bool {
        true
    }
    fn has_discard(&self) -> bool {
        true
    }

    fn choose(
        &mut self,
        g: &Game,
        c: &Card,
        kind: &str,
        ops: &[Value],
        pending: Option<&Effect>,
    ) -> Result<Map<String, Value>, Illegal> {
        if c.owner == 1 {
            return self.ai.choose(g, c, kind, ops, pending);
        }
        let raw_opts = choices_for(g, c, kind, ops, pending);
        if raw_opts.len() == 1 {
            return Ok(raw_opts[0].1.clone());
        }
        if raw_opts.is_empty() {
            return Ok(Map::new());
        }
        let options: Vec<(Map<String, Value>, OptionPayload)> = raw_opts
            .into_iter()
            .map(|(v, m)| (v.as_object().unwrap().clone(), OptionPayload::ChoiceMap(m)))
            .collect();
        let payment_due = if pending.is_some() { c.cost() } else { 0 };
        let ans = ask_human(
            &self.shared,
            g,
            self.seed,
            &self.generation,
            "choice",
            &format!("Choose {}: {kind}", c.title()),
            options,
            Some(c),
            None,
            None,
            None,
            payment_due,
            None,
        )?;
        match ans {
            AnswerValue::Option(OptionPayload::ChoiceMap(m)) => Ok(m),
            _ => Ok(Map::new()),
        }
    }

    fn respond(
        &mut self,
        g: &Game,
        pending: &Effect,
        is_reply: bool,
    ) -> Result<Option<Vec<Response>>, Illegal> {
        let owner = 1 - pending.source.owner;
        if owner == 1 {
            return self.ai.respond(g, pending, is_reply);
        }
        let mut options: Vec<(Map<String, Value>, OptionPayload)> = Vec::new();
        let mut pass_map = Map::new();
        pass_map.insert("label".to_string(), json!("Pass this response window"));
        pass_map.insert("action".to_string(), json!("pass"));
        options.push((pass_map, OptionPayload::ResponseUid(None)));

        for c in &g.p[0].hand {
            if c.cost() > g.p[0].actions
                || (c.card_type() != "Counter" && (is_reply || !c.response_play()))
            {
                continue;
            }
            let raw = if c.card_type() == "Character" {
                g.entry_ops(c)?
            } else {
                c.spec
                    .get("ops")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
            };
            if !choices_for(g, c, "counter", &raw, Some(pending)).is_empty() {
                let mut m = Map::new();
                m.insert(
                    "label".to_string(),
                    json!(format!("Play {} · {} Actions", c.title(), c.cost())),
                );
                m.insert("action".to_string(), json!("respond"));
                m.insert("card_uid".to_string(), json!(c.uid));
                m.insert("card_id".to_string(), json!(c.id()));
                options.push((m, OptionPayload::ResponseUid(Some(c.uid))));
            }
        }

        let target = pending
            .choice
            .get("target")
            .and_then(as_strict_i32)
            .and_then(|u| g.cards.get(&u));
        let public_target = target.filter(|tc| {
            g.p.iter().any(|p| {
                p.field.iter().any(|x| x.uid == tc.uid) || p.discard.iter().any(|x| x.uid == tc.uid)
            })
        });
        let mode_str = pending.choice.get("mode").and_then(|v| v.as_str());
        let effect_json = json!({
            "kind": pending.kind,
            "mode": pending.choice.get("mode").cloned().unwrap_or(Value::Null),
            "mode_label": mode_str.map(|m| json!(mode_label(Some(m)))).unwrap_or(Value::Null),
            "ops": pending.ops,
            "target": public_target.map(serialize_card).unwrap_or(Value::Null),
            "is_reply": is_reply,
            "parent_source": pending.target_effect.as_ref().map(|te| serialize_card(&te.source)).unwrap_or(Value::Null)
        });
        let ans = ask_human(
            &self.shared,
            g,
            self.seed,
            &self.generation,
            "response",
            &format!("Respond to {}: {}", pending.source.title(), pending.kind),
            options,
            Some(&pending.source),
            None,
            None,
            Some(effect_json),
            0,
            None,
        )?;
        match ans {
            AnswerValue::Option(OptionPayload::ResponseUid(Some(uid))) => {
                Ok(Some(vec![Response::new(uid)]))
            }
            _ => Ok(None),
        }
    }

    fn pick(&mut self, g: &Game, owner: usize, cards: &[Card]) -> Result<i32, Illegal> {
        if owner == 1 {
            let best = cards
                .iter()
                .max_by(|a, b| {
                    card_value(a)
                        .partial_cmp(&card_value(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.uid.cmp(&a.uid))
                })
                .unwrap();
            return Ok(best.uid);
        }
        let options: Vec<(Map<String, Value>, OptionPayload)> = cards
            .iter()
            .map(|c| {
                let mut m = Map::new();
                m.insert("label".to_string(), json!(c.title()));
                m.insert("card_uid".to_string(), json!(c.uid));
                m.insert("card_id".to_string(), json!(c.id()));
                m.insert("card".to_string(), serialize_card(c));
                (m, OptionPayload::PickUid(c.uid))
            })
            .collect();
        let ans = ask_human(
            &self.shared,
            g,
            self.seed,
            &self.generation,
            "pick",
            "Look at these cards; choose one for your hand",
            options,
            None,
            None,
            None,
            None,
            0,
            None,
        )?;
        match ans {
            AnswerValue::Option(OptionPayload::PickUid(uid)) => Ok(uid),
            _ => Ok(cards[0].uid),
        }
    }

    fn discard(&mut self, g: &Game, owner: usize, n: usize) -> Result<Vec<i32>, Illegal> {
        self.discard_with_reason(g, owner, n, "effect")
    }
}

impl MatchProviders {
    fn discard_with_reason(
        &mut self,
        g: &Game,
        owner: usize,
        n: usize,
        reason: &str,
    ) -> Result<Vec<i32>, Illegal> {
        let hand = &g.p[owner].hand;
        let count = n.min(hand.len());
        if owner == 1 {
            let mut sorted = hand.clone();
            sorted.sort_by(|a, b| {
                card_value(a)
                    .partial_cmp(&card_value(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.uid.cmp(&b.uid))
            });
            return Ok(sorted.into_iter().take(count).map(|c| c.uid).collect());
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let title = if reason == "hand_limit" {
            format!("Hand limit: discard {count} to keep six")
        } else {
            format!("Card effect: discard {count} card(s)")
        };
        let ans = ask_human(
            &self.shared,
            g,
            self.seed,
            &self.generation,
            "discard",
            &title,
            Vec::new(),
            None,
            Some(count),
            Some(hand),
            None,
            0,
            Some(reason),
        )?;
        match ans {
            AnswerValue::SelectedCards(uids) => Ok(uids),
            _ => Ok(Vec::new()),
        }
    }
}

impl Match {
    pub fn new(seed: i64, starter: i64) -> Result<Self, String> {
        if !(0..=2_147_483_647).contains(&seed) {
            return Err("Seed must be an integer from 0 to 2147483647".to_string());
        }
        if starter != 0 && starter != 1 {
            return Err("Starter must be 0 or 1".to_string());
        }
        let seed_u64 = seed as u64;
        let starter_usize = starter as usize;
        let generation = next_generation_hex();
        let mut g = stock_game(seed_u64)?;
        let initial = snapshot(&g)?;
        g.trace_transitions = true;

        let shared = Arc::new((
            Mutex::new(SharedMatchState {
                stopped: false,
                answer: None,
                pending: None,
                revision: 0,
                decision_count: 0,
                inputs: Vec::new(),
                error: None,
                internal_error: None,
                cached: json!({}),
                transitions: Vec::new(),
                log: Vec::new(),
                winner: None,
            }),
            Condvar::new(),
        ));

        // Publish authoritative starting view before worker thread can race it.
        publish_state(&shared, &g, seed_u64, &generation, "thinking", None);

        let shared_clone = Arc::clone(&shared);
        let gen_clone = generation.clone();
        thread::Builder::new()
            .name(format!("wonderland-match-{}", &generation[..6]))
            .spawn(move || {
                Self::worker_loop(shared_clone, g, seed_u64, starter_usize, gen_clone);
            })
            .map_err(|e| e.to_string())?;

        Ok(Self {
            seed: seed_u64,
            starter: starter_usize,
            generation,
            initial,
            shared,
        })
    }

    fn worker_loop(
        shared: Arc<(Mutex<SharedMatchState>, Condvar)>,
        mut g: Game,
        seed: u64,
        starter: usize,
        generation: String,
    ) {
        let mut mp = MatchProviders {
            shared: Arc::clone(&shared),
            seed,
            generation: generation.clone(),
            ai: AuditPolicy::new(["mixed", "mixed"]),
        };
        let mut owner = starter;
        loop {
            {
                let (lock, _) = &*shared;
                if lock.lock().unwrap().stopped {
                    return;
                }
            }
            if let Err(e) = g.start_with_providers(owner, &mp) {
                Self::handle_worker_err(&shared, &g, seed, &generation, e);
                return;
            }
            let turn_res = if owner == 0 {
                Self::human_turn(&mut g, &mut mp)
            } else {
                Self::ai_turn(&mut g, &mut mp)
            };
            if let Err(e) = turn_res {
                Self::handle_worker_err(&shared, &g, seed, &generation, e);
                return;
            }
            if g.winner.is_some() {
                let _ = g.end_turn_with_providers(&[], &mp);
                publish_state(&shared, &g, seed, &generation, "finished", None);
                return;
            }
            let excess = g.p[owner].hand.len().saturating_sub(6);
            let discard = if excess > 0 {
                match mp.discard_with_reason(&g, owner, excess, "hand_limit") {
                    Ok(d) => d,
                    Err(e) => {
                        Self::handle_worker_err(&shared, &g, seed, &generation, e);
                        return;
                    }
                }
            } else {
                Vec::new()
            };
            if let Err(e) = g.end_turn_with_providers(&discard, &mp) {
                Self::handle_worker_err(&shared, &g, seed, &generation, e);
                return;
            }
            owner = 1 - owner;
            publish_state(&shared, &g, seed, &generation, "thinking", None);
        }
    }

    fn handle_worker_err(
        shared: &Arc<(Mutex<SharedMatchState>, Condvar)>,
        g: &Game,
        seed: u64,
        generation: &str,
        err: Illegal,
    ) {
        if err.0 == "STOPPED" {
            return;
        }
        {
            let (lock, _) = &**shared;
            let mut st = lock.lock().unwrap();
            st.error =
                Some("Illegal: Match stopped safely. Restart or export diagnostics.".to_string());
            st.internal_error = Some(format!("Illegal({:?})", err.0));
        }
        publish_state(shared, g, seed, generation, "error", None);
    }

    fn human_turn(g: &mut Game, mp: &mut MatchProviders) -> Result<(), Illegal> {
        while g.winner.is_none() {
            let mut options: Vec<(Map<String, Value>, OptionPayload)> = Vec::new();
            if g.phase == "placement" {
                for c in &g.p[0].hand {
                    if c.card_type() != "Counter" && c.cost() <= g.p[0].actions {
                        let mut m = Map::new();
                        m.insert("label".to_string(), json!(format!("Play {}", c.title())));
                        m.insert("action".to_string(), json!("play"));
                        m.insert("card_uid".to_string(), json!(c.uid));
                        m.insert("card_id".to_string(), json!(c.id()));
                        options.push((
                            m,
                            OptionPayload::TurnAction("play".to_string(), Some(c.uid)),
                        ));
                    }
                }
                let mut close_m = Map::new();
                close_m.insert("label".to_string(), json!("Resolve this sequence"));
                close_m.insert("action".to_string(), json!("close_sequence"));
                options.push((
                    close_m,
                    OptionPayload::TurnAction("close".to_string(), None),
                ));
                if g.sequence.is_empty() {
                    let mut end_m = Map::new();
                    end_m.insert("label".to_string(), json!("End turn"));
                    end_m.insert("action".to_string(), json!("end_turn"));
                    options.push((end_m, OptionPayload::TurnAction("end".to_string(), None)));
                }
            } else {
                let mut begin_m = Map::new();
                begin_m.insert("label".to_string(), json!("Build another sequence"));
                begin_m.insert("action".to_string(), json!("begin_sequence"));
                options.push((
                    begin_m,
                    OptionPayload::TurnAction("begin".to_string(), None),
                ));
                let mut end_m = Map::new();
                end_m.insert("label".to_string(), json!("End turn"));
                end_m.insert("action".to_string(), json!("end_turn"));
                options.push((end_m, OptionPayload::TurnAction("end".to_string(), None)));
            }
            let ans = ask_human(
                &mp.shared,
                g,
                mp.seed,
                &mp.generation,
                "turn",
                "Build left to right, then resolve; keep Actions for responses.",
                options,
                None,
                None,
                None,
                None,
                0,
                None,
            )?;
            if let AnswerValue::Option(OptionPayload::TurnAction(action, uid_opt)) = ans {
                match action.as_str() {
                    "play" => {
                        if let Some(u) = uid_opt {
                            g.place_with_providers(u, mp)?;
                        }
                    }
                    "close" => {
                        g.close_with_providers(mp)?;
                    }
                    "begin" => {
                        g.begin_sequence_with_providers(mp)?;
                    }
                    "end" => {
                        if g.phase == "placement" {
                            g.close_with_providers(mp)?;
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn ai_turn(g: &mut Game, mp: &mut MatchProviders) -> Result<(), Illegal> {
        while g.winner.is_none() {
            {
                let (lock, _) = &*mp.shared;
                if lock.lock().unwrap().stopped {
                    return Err(Illegal("STOPPED".into()));
                }
            }
            let chosen = mp.ai.sequence(g, 1);
            if chosen.is_empty() {
                if g.phase == "placement" {
                    g.close_with_providers(mp)?;
                }
                break;
            }
            if g.phase == "resolved" {
                g.begin_sequence_with_providers(mp)?;
            }
            for c in chosen {
                g.place_with_providers(c.uid, mp)?;
            }
            g.close_with_providers(mp)?;
        }
        Ok(())
    }

    pub fn state(&self) -> Value {
        let (lock, _) = &*self.shared;
        let st = lock.lock().unwrap();
        st.cached.clone()
    }

    pub fn submit(&self, data: &Value) -> Result<Value, String> {
        let obj = data
            .as_object()
            .ok_or_else(|| "Expected an object".to_string())?;
        let (lock, cvar) = &*self.shared;
        let mut st = lock.lock().unwrap();
        if st.stopped || st.pending.is_none() || st.answer.is_some() {
            return Err("This decision is stale; refresh the table".to_string());
        }
        let p = st.pending.as_ref().unwrap();
        let want_id = p.public.get("id").and_then(|v| v.as_str());
        if obj.get("decision_id").and_then(|v| v.as_str()) != want_id {
            return Err("This decision is stale; refresh the table".to_string());
        }
        let ans = if let Some(req_cnt) = p.count {
            let sel_arr = obj
                .get("selected")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "Select exactly the requested cards".to_string())?;
            let mut uids = Vec::new();
            let mut seen = HashSet::new();
            for x in sel_arr {
                let u = as_strict_i32(x)
                    .ok_or_else(|| "Select exactly the requested cards".to_string())?;
                if !seen.insert(u) || !p.cards.contains(&u) {
                    return Err("Select exactly the requested cards".to_string());
                }
                uids.push(u);
            }
            if uids.len() != req_cnt {
                return Err("Select exactly the requested cards".to_string());
            }
            AnswerValue::SelectedCards(uids)
        } else {
            let oid = obj
                .get("option_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Choose one of the available options".to_string())?;
            let payload = p
                .values
                .get(oid)
                .cloned()
                .ok_or_else(|| "Choose one of the available options".to_string())?;
            AnswerValue::Option(payload)
        };
        st.inputs.push(data.clone());
        st.answer = Some(ans);
        cvar.notify_all();
        Ok(json!({"accepted": true}))
    }

    pub fn stop(&self) {
        let (lock, cvar) = &*self.shared;
        let mut st = lock.lock().unwrap();
        st.stopped = true;
        st.pending = None;
        cvar.notify_all();
    }

    pub fn replay(&self) -> Result<Value, String> {
        let (lock, _) = &*self.shared;
        let st = lock.lock().unwrap();
        let status = st
            .cached
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if status != "finished" && status != "error" {
            return Err(
                "Full debug replay is available only after the match ends; it includes concealed cards"
                    .to_string(),
            );
        }
        Ok(json!({
            "schema": "wonderland-digital-replay-v1",
            "seed": self.seed,
            "starter": self.starter,
            "pool_sha256": pool_sha256(),
            "runtime_sha256": runtime_hashes(),
            "initial_state": self.initial,
            "inputs": st.inputs,
            "transitions": st.transitions,
            "log": st.log,
            "winner": st.winner,
            "error": st.internal_error
        }))
    }
}

impl Drop for Match {
    fn drop(&mut self) {
        self.stop();
    }
}
