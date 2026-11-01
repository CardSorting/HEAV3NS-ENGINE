use crate::engine::{
    as_strict_i32, is_strict_int, specs_hash, Card, DecisionProviders, Effect, Game, Illegal,
    Response,
};
use serde_json::{json, Map, Value};
use std::collections::{HashSet, VecDeque};

pub const ZONES: &[&str] = &["hand", "deck", "field", "shields", "discard"];

/// Produces a `wonderland-state-2` boundary snapshot matching `r2_contract.snapshot(g)`.
pub fn snapshot(g: &Game) -> Result<Value, String> {
    if g.phase == "resolving" {
        return Err("Snapshots require a public command boundary".to_string());
    }
    let cards: Vec<Value> = g
        .cards
        .values()
        .map(|c| {
            json!({
                "uid": c.uid,
                "owner": c.owner,
                "id": c.id(),
                "cancelled": c.cancelled,
                "combo_used": c.combo_used
            })
        })
        .collect();
    let players: Vec<Value> =
        g.p.iter()
            .map(|p| {
                let mut m = Map::new();
                m.insert("actions".to_string(), json!(p.actions));
                for &z in ZONES {
                    let ids: Vec<i32> = p.zone(z).iter().map(|c| c.uid).collect();
                    m.insert(z.to_string(), json!(ids));
                }
                Value::Object(m)
            })
            .collect();
    let seq: Vec<i32> = g.sequence.iter().map(|c| c.uid).collect();
    Ok(json!({
        "schema": "wonderland-state-2",
        "rng_state": g.rng.getstate_json(),
        "specs_sha256": specs_hash(&g.specs_list),
        "turn_number": g.turn_number,
        "active": g.active,
        "phase": g.phase,
        "winner": g.winner,
        "opening_protection": g.opening_protection,
        "cards": cards,
        "players": players,
        "sequence": seq,
        "log": g.log
    }))
}

/// Restores a `Game` (or `SimGame` when `sim_mode` is true) from `wonderland-state-1` or `wonderland-state-2` with full invariant checking.
pub fn restore(
    state: &Value,
    specs: Vec<Value>,
    allow_legacy_rng: bool,
    sim_mode: bool,
) -> Result<Game, String> {
    let obj = state
        .as_object()
        .ok_or_else(|| "Unknown snapshot schema".to_string())?;
    let schema = obj
        .get("schema")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Unknown snapshot schema".to_string())?;
    if schema != "wonderland-state-1" && schema != "wonderland-state-2" {
        return Err("Unknown snapshot schema".to_string());
    }
    if schema == "wonderland-state-1" && !allow_legacy_rng {
        return Err(
            "Legacy snapshot lacks RNG state; exact restoration unavailable. Explicit allow_legacy_rng=True initializes seed0 for fixture import only."
                .to_string(),
        );
    }
    let mut required: HashSet<&str> = [
        "schema",
        "specs_sha256",
        "turn_number",
        "active",
        "phase",
        "winner",
        "opening_protection",
        "cards",
        "players",
        "sequence",
        "log",
    ]
    .into_iter()
    .collect();
    if schema == "wonderland-state-2" {
        required.insert("rng_state");
    }
    let state_keys: HashSet<&str> = obj.keys().map(|s| s.as_str()).collect();
    if state_keys != required {
        return Err("Snapshot fields mismatch".to_string());
    }

    let cards_arr = obj
        .get("cards")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid snapshot collections".to_string())?;
    let players_arr = obj
        .get("players")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid snapshot collections".to_string())?;
    let log_arr = obj
        .get("log")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid snapshot collections".to_string())?;

    for ev in log_arr {
        let ev_arr = ev
            .as_array()
            .ok_or_else(|| "Invalid event log".to_string())?;
        if ev_arr.is_empty() || !ev_arr[0].is_string() {
            return Err("Invalid event log".to_string());
        }
    }

    let card_req: HashSet<&str> = ["uid", "owner", "id", "cancelled", "combo_used"]
        .into_iter()
        .collect();
    for row in cards_arr {
        let ro = row
            .as_object()
            .ok_or_else(|| "Invalid card record".to_string())?;
        let rk: HashSet<&str> = ro.keys().map(|s| s.as_str()).collect();
        if rk != card_req {
            return Err("Invalid card record".to_string());
        }
        if !ro.get("id").map(|v| v.is_string()).unwrap_or(false) {
            return Err("Invalid card definition id".to_string());
        }
    }

    let mut player_req: HashSet<&str> = ZONES.iter().copied().collect();
    player_req.insert("actions");
    for row in players_arr {
        let ro = row
            .as_object()
            .ok_or_else(|| "Invalid player record".to_string())?;
        let rk: HashSet<&str> = ro.keys().map(|s| s.as_str()).collect();
        if rk != player_req {
            return Err("Invalid player record".to_string());
        }
    }

    let mut g = if sim_mode {
        Game::new_sim(Some(specs.clone()), false).map_err(|e| e.0)?
    } else {
        Game::new(Some(specs.clone()), false).map_err(|e| e.0)?
    };

    if schema == "wonderland-state-2" {
        let r = obj.get("rng_state").unwrap();
        g.rng
            .setstate_json(r)
            .map_err(|_| "Invalid RNG state".to_string())?;
    }

    let want_hash = specs_hash(&specs);
    if obj.get("specs_sha256").and_then(|v| v.as_str()) != Some(want_hash.as_str()) {
        return Err("Snapshot card-definition hash mismatch".to_string());
    }

    let phase = obj.get("phase").and_then(|v| v.as_str()).unwrap_or("");
    if !matches!(phase, "ended" | "placement" | "resolved") {
        return Err("Invalid boundary phase".to_string());
    }

    let parse_opt_seat = |key: &str| -> Result<Option<usize>, String> {
        let v = obj.get(key).unwrap();
        if v.is_null() {
            Ok(None)
        } else {
            let s = as_strict_i32(v).ok_or_else(|| format!("Invalid {key}"))?;
            if s != 0 && s != 1 {
                return Err(format!("Invalid {key}"));
            }
            Ok(Some(s as usize))
        }
    };
    let active = parse_opt_seat("active")?;
    let winner = parse_opt_seat("winner")?;

    let turn_number = obj
        .get("turn_number")
        .and_then(as_strict_i32)
        .ok_or_else(|| "Invalid turn number".to_string())?;
    if turn_number < 0 {
        return Err("Invalid turn number".to_string());
    }
    let opening_protection = obj
        .get("opening_protection")
        .and_then(|v| v.as_bool())
        .ok_or_else(|| "Invalid protection flag".to_string())?;

    g.turn_number = turn_number;
    g.active = active;
    g.winner = winner;
    g.phase = phase.to_string();
    g.opening_protection = opening_protection;

    for row in cards_arr {
        let ro = row.as_object().unwrap();
        let uid = ro
            .get("uid")
            .and_then(as_strict_i32)
            .ok_or_else(|| "Invalid or duplicate card UID/owner".to_string())?;
        let owner = ro
            .get("owner")
            .and_then(as_strict_i32)
            .ok_or_else(|| "Invalid or duplicate card UID/owner".to_string())?;
        if uid < 1 || g.cards.contains_key(&uid) || (owner != 0 && owner != 1) {
            return Err("Invalid or duplicate card UID/owner".to_string());
        }
        let cid = ro.get("id").and_then(|v| v.as_str()).unwrap();
        let spec = g
            .specs
            .get(cid)
            .cloned()
            .ok_or_else(|| "Invalid card definition/flags".to_string())?;
        let cancelled = ro
            .get("cancelled")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| "Invalid card definition/flags".to_string())?;
        let combo_used = ro
            .get("combo_used")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| "Invalid card definition/flags".to_string())?;
        let mut c = Card::new(uid, owner as usize, spec);
        c.cancelled = cancelled;
        c.combo_used = combo_used;
        g.cards.insert(uid, c);
    }

    if players_arr.len() != 2 {
        return Err("Exactly two players required".to_string());
    }
    let mut seen = Vec::new();
    for (owner, row) in players_arr.iter().enumerate() {
        let ro = row.as_object().unwrap();
        let actions = ro
            .get("actions")
            .and_then(as_strict_i32)
            .ok_or_else(|| "Actions outside 0..6".to_string())?;
        if !(0..=6).contains(&actions) {
            return Err("Actions outside 0..6".to_string());
        }
        g.p[owner].actions = actions;
        for &z in ZONES {
            let ids_arr = ro
                .get(z)
                .and_then(|v| v.as_array())
                .ok_or_else(|| "Invalid zone membership".to_string())?;
            let mut zone_cards = Vec::new();
            for u_val in ids_arr {
                let u =
                    as_strict_i32(u_val).ok_or_else(|| "Invalid zone membership".to_string())?;
                let c = g
                    .cards
                    .get(&u)
                    .cloned()
                    .ok_or_else(|| "Invalid zone membership".to_string())?;
                if c.owner != owner {
                    return Err("Invalid zone membership".to_string());
                }
                seen.push(u);
                zone_cards.push(c);
            }
            *g.p[owner].zone_mut(z) = zone_cards;
        }
    }
    let seen_set: HashSet<i32> = seen.iter().copied().collect();
    let card_keys: HashSet<i32> = g.cards.keys().copied().collect();
    if seen.len() != seen_set.len() || seen_set != card_keys {
        return Err("Card conservation failure".to_string());
    }

    let seq_arr = obj
        .get("sequence")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Invalid sequence".to_string())?;
    let mut seq_uids = Vec::new();
    let mut seq_set = HashSet::new();
    for u_val in seq_arr {
        let u = as_strict_i32(u_val).ok_or_else(|| "Invalid sequence".to_string())?;
        if !g.cards.contains_key(&u) || !seq_set.insert(u) {
            return Err("Invalid sequence".to_string());
        }
        seq_uids.push(u);
    }

    if (g.phase == "placement" || g.phase == "resolved") && g.active.is_none() {
        return Err("Active player required".to_string());
    }
    if let Some(w) = g.winner {
        if !g.p[1 - w].shields.is_empty() {
            return Err("Winner inconsistent with Shields".to_string());
        }
    }
    if seq_uids.iter().any(|u| Some(g.cards[u].owner) != g.active) {
        return Err("Sequence ownership mismatch".to_string());
    }
    if (g.turn_number == 0) != g.active.is_none() {
        return Err("Turn/active mismatch".to_string());
    }
    if g.turn_number == 0 && (g.phase != "ended" || !seq_uids.is_empty() || g.winner.is_some()) {
        return Err("Invalid setup boundary".to_string());
    }
    if g.phase == "ended" && g.turn_number > 0 && g.winner.is_none() {
        let act = g.active.unwrap();
        if g.p[act].hand.len() > 6 {
            return Err("Unfinished end-turn hand cleanup".to_string());
        }
    }
    if seq_uids.iter().any(|u| g.cards[u].card_type() == "Counter") {
        return Err("Counter in ordinary sequence".to_string());
    }
    if g.phase == "placement" {
        let act = g.active.unwrap();
        let total_cost: i32 = seq_uids.iter().map(|u| g.cards[u].cost()).sum();
        if total_cost > 6 - g.p[act].actions {
            return Err("Placement costs exceed spent Actions".to_string());
        }
        let active_field_uids: Vec<i32> = g.p[act].field.iter().map(|c| c.uid).collect();
        if g.winner.is_some() || !g.p[1 - act].field.is_empty() || active_field_uids != seq_uids {
            return Err("Placement sequence/field mismatch".to_string());
        }
    } else {
        if g.p.iter().any(|p| !p.field.is_empty()) {
            return Err("Orphan boundary field cards".to_string());
        }
        if let Some(act) = g.active {
            for u in &seq_uids {
                let in_hand_or_disc = g.p[act].hand.iter().any(|c| c.uid == *u)
                    || g.p[act].discard.iter().any(|c| c.uid == *u);
                if !in_hand_or_disc {
                    return Err("Resolved sequence zone mismatch".to_string());
                }
            }
        }
    }

    g.sequence = seq_uids.iter().map(|u| g.cards[u].clone()).collect();
    g.log = log_arr
        .iter()
        .map(|ev| ev.as_array().unwrap().clone())
        .collect();
    g.invariants().map_err(|e| e.0)?;
    Ok(g)
}

fn parse_recorded_response(val: &Value) -> Result<Option<Vec<Response>>, String> {
    if val.is_null() {
        return Ok(None);
    }
    if let Some(arr) = val.as_array() {
        let mut out = Vec::new();
        for item in arr {
            let parsed = parse_single_response(item)?;
            out.push(parsed);
        }
        return Ok(Some(out));
    }
    Ok(Some(vec![parse_single_response(val)?]))
}

fn parse_single_response(val: &Value) -> Result<Response, String> {
    let obj = val
        .as_object()
        .ok_or_else(|| "Malformed recorded Response".to_string())?;
    let keys: HashSet<&str> = obj.keys().map(|s| s.as_str()).collect();
    let want: HashSet<&str> = ["uid", "reply"].into_iter().collect();
    if keys != want {
        return Err("Malformed recorded Response".to_string());
    }
    let uid = obj
        .get("uid")
        .and_then(as_strict_i32)
        .ok_or_else(|| "Malformed recorded Response".to_string())?;
    let reply = parse_recorded_response(obj.get("reply").unwrap())?;
    Ok(Response { uid, reply })
}

struct ReplayProviders {
    roles: HashSet<String>,
    queue: VecDeque<(String, Value)>,
    cmd_index: usize,
    error: Option<String>,
}

impl ReplayProviders {
    fn pop_role(&mut self, role: &str) -> Result<Value, Illegal> {
        match self.queue.front() {
            Some((r, _)) if r == role => {
                let (_, v) = self.queue.pop_front().unwrap();
                Ok(v)
            }
            _ => {
                let msg = format!(
                    "Missing/reordered {role} decision at command {}",
                    self.cmd_index
                );
                self.error = Some(msg.clone());
                Err(Illegal(msg))
            }
        }
    }
}

impl DecisionProviders for ReplayProviders {
    fn has_choice(&self) -> bool {
        self.roles.contains("choice")
    }
    fn has_response(&self) -> bool {
        self.roles.contains("response")
    }
    fn has_pick(&self) -> bool {
        self.roles.contains("pick")
    }
    fn has_discard(&self) -> bool {
        self.roles.contains("discard")
    }

    fn choose(
        &mut self,
        _g: &Game,
        _c: &Card,
        _kind: &str,
        _ops: &[Value],
        _pending: Option<&Effect>,
    ) -> Result<Map<String, Value>, Illegal> {
        let val = self.pop_role("choice")?;
        val.as_object()
            .cloned()
            .ok_or_else(|| Illegal("Malformed choice output".into()))
    }

    fn respond(
        &mut self,
        _g: &Game,
        _pending: &Effect,
        _is_reply: bool,
    ) -> Result<Option<Vec<Response>>, Illegal> {
        let val = self.pop_role("response")?;
        parse_recorded_response(&val).map_err(|e| {
            self.error = Some(e.clone());
            Illegal(e)
        })
    }

    fn pick(&mut self, _g: &Game, _owner: usize, _cards: &[Card]) -> Result<i32, Illegal> {
        let val = self.pop_role("pick")?;
        as_strict_i32(&val).ok_or_else(|| Illegal("Malformed pick output".into()))
    }

    fn discard(&mut self, _g: &Game, _owner: usize, _n: usize) -> Result<Vec<i32>, Illegal> {
        let val = self.pop_role("discard")?;
        let arr = val
            .as_array()
            .ok_or_else(|| Illegal("Malformed discard output".into()))?;
        let mut out = Vec::new();
        for item in arr {
            let u = as_strict_i32(item).ok_or_else(|| Illegal("Malformed discard uid".into()))?;
            out.push(u);
        }
        Ok(out)
    }
}

/// Replays committed `wonderland-transition-1` commands on `SimGame` matching `replay_transitions.replay`.
pub fn replay_transitions(
    specs: Vec<Value>,
    initial_state: &Value,
    transitions: &Value,
) -> Result<Game, String> {
    let trans_arr = transitions
        .as_array()
        .ok_or_else(|| "Transitions must be a list".to_string())?;
    let mut g = restore(initial_state, specs, false, true)?;
    g.trace_transitions = true;

    for (idx0, expected) in trans_arr.iter().enumerate() {
        let index = idx0 + 1;
        let exp_obj = expected
            .as_object()
            .ok_or_else(|| "Malformed transition record".to_string())?;
        if exp_obj.get("schema").and_then(|v| v.as_str()) != Some("wonderland-transition-1")
            || exp_obj.get("status").and_then(|v| v.as_str()) != Some("committed")
        {
            return Err("Command replay requires committed schema1 transitions".to_string());
        }
        let outputs_arr = exp_obj
            .get("provider_outputs")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Malformed recorded provider outputs".to_string())?;
        let mut queue = VecDeque::new();
        let out_req: HashSet<&str> = ["role", "output"].into_iter().collect();
        for r in outputs_arr {
            let ro = r
                .as_object()
                .ok_or_else(|| "Malformed recorded provider outputs".to_string())?;
            let rk: HashSet<&str> = ro.keys().map(|s| s.as_str()).collect();
            if rk != out_req {
                return Err("Malformed recorded provider outputs".to_string());
            }
            let role = ro
                .get("role")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Malformed recorded provider outputs".to_string())?;
            if !matches!(role, "choice" | "response" | "pick" | "discard") {
                return Err("Malformed recorded provider outputs".to_string());
            }
            queue.push_back((role.to_string(), ro.get("output").unwrap().clone()));
        }

        let declared_arr = exp_obj
            .get("provider_roles")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Malformed provider-role declarations".to_string())?;
        let mut roles = HashSet::new();
        for r_val in declared_arr {
            let r_str = r_val
                .as_str()
                .ok_or_else(|| "Malformed provider-role declarations".to_string())?;
            if !matches!(r_str, "choice" | "response" | "pick" | "discard")
                || !roles.insert(r_str.to_string())
            {
                return Err("Malformed provider-role declarations".to_string());
            }
        }
        if queue.iter().any(|(r, _)| !roles.contains(r)) {
            return Err("Undeclared provider output".to_string());
        }

        let command = exp_obj
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Unknown replay command".to_string())?;
        if !matches!(
            command,
            "start"
                | "begin_sequence"
                | "place"
                | "close"
                | "end_turn"
                | "add"
                | "draw"
                | "break_shields"
        ) {
            return Err("Unknown replay command".to_string());
        }
        let selected = exp_obj
            .get("selected")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "Malformed selected command".to_string())?;
        let args = selected
            .get("args")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Malformed selected command".to_string())?;
        let kwargs = selected
            .get("kwargs")
            .and_then(|v| v.as_object())
            .ok_or_else(|| "Malformed selected command".to_string())?;
        if !kwargs.is_empty() {
            return Err(
                "Scripted keyword command replay is unsupported; use captured providers"
                    .to_string(),
            );
        }

        let mut rp = ReplayProviders {
            roles,
            queue,
            cmd_index: index,
            error: None,
        };

        let cmd_res: Result<(), Illegal> = match command {
            "start" => {
                if args.len() != 1 || !is_strict_int(&args[0]) {
                    Err(Illegal("invalid start args".into()))
                } else {
                    let owner = as_strict_i32(&args[0]).unwrap() as usize;
                    g.start_with_providers(owner, &rp)
                }
            }
            "begin_sequence" => {
                if !args.is_empty() {
                    Err(Illegal("invalid begin_sequence args".into()))
                } else {
                    g.begin_sequence_with_providers(&rp)
                }
            }
            "place" => {
                if args.len() != 1 || !is_strict_int(&args[0]) {
                    Err(Illegal("invalid place args".into()))
                } else {
                    let uid = as_strict_i32(&args[0]).unwrap();
                    g.place_with_providers(uid, &rp)
                }
            }
            "close" => {
                if !args.is_empty() {
                    Err(Illegal("invalid close args".into()))
                } else {
                    g.close_with_providers(&mut rp)
                }
            }
            "end_turn" => {
                if args.len() > 1 {
                    Err(Illegal("invalid end_turn args".into()))
                } else if args.is_empty() {
                    g.end_turn_with_providers(&[], &rp)
                } else {
                    let disc_arr = args[0]
                        .as_array()
                        .ok_or_else(|| Illegal("invalid discard list".into()))?;
                    let mut uids = Vec::new();
                    for x in disc_arr {
                        let u = as_strict_i32(x)
                            .ok_or_else(|| Illegal("invalid discard list".into()))?;
                        uids.push(u);
                    }
                    g.end_turn_with_providers(&uids, &rp)
                }
            }
            "add" => {
                if args.len() < 2
                    || args.len() > 3
                    || !is_strict_int(&args[0])
                    || !args[1].is_string()
                {
                    Err(Illegal("invalid add args".into()))
                } else {
                    let owner = as_strict_i32(&args[0]).unwrap() as usize;
                    let cid = args[1].as_str().unwrap();
                    let zone = if args.len() == 3 {
                        args[2]
                            .as_str()
                            .ok_or_else(|| Illegal("invalid zone".into()))?
                    } else {
                        "hand"
                    };
                    g.add(owner, cid, zone).map(|_| ())
                }
            }
            "draw" => {
                if args.len() != 2 || !is_strict_int(&args[0]) || !is_strict_int(&args[1]) {
                    Err(Illegal("invalid draw args".into()))
                } else {
                    let owner = as_strict_i32(&args[0]).unwrap() as usize;
                    let n = as_strict_i32(&args[1]).unwrap();
                    g.draw(owner, n)
                }
            }
            "break_shields" => {
                if args.len() != 2 || !is_strict_int(&args[0]) || !is_strict_int(&args[1]) {
                    Err(Illegal("invalid break_shields args".into()))
                } else {
                    let owner = as_strict_i32(&args[0]).unwrap() as usize;
                    let n = as_strict_i32(&args[1]).unwrap();
                    g.break_shields(owner, n)
                }
            }
            _ => Err(Illegal("Unknown replay command".into())),
        };

        if let Err(e) = cmd_res {
            return Err(format!(
                "Command replay failed at {index}: Illegal: {}",
                e.0
            ));
        }
        if !rp.queue.is_empty() {
            return Err(format!("Unused decisions at command {index}"));
        }
        let actual = g.transitions.last().unwrap();
        let actual_rng = actual
            .get("after")
            .and_then(|a| a.get("rng_sha256"))
            .and_then(|v| v.as_str());
        let expected_rng = exp_obj
            .get("after")
            .and_then(|a| a.get("rng_sha256"))
            .and_then(|v| v.as_str());
        if actual_rng != expected_rng {
            return Err(
                "Owned RNG progression divergence: custom RNG-consuming provider command replay is unsupported"
                    .to_string(),
            );
        }
        if actual != expected {
            return Err(format!("Transition divergence at command {index}"));
        }
    }
    Ok(g)
}

/// Verifies a `wonderland-digital-replay-v1` document matching `replay_check.verify_replay_data`.
pub fn verify_replay_data(data: &Value) -> Result<Value, String> {
    let obj = data
        .as_object()
        .ok_or_else(|| "Unsupported replay schema".to_string())?;
    if obj.get("schema").and_then(|v| v.as_str()) != Some("wonderland-digital-replay-v1") {
        return Err("Unsupported replay schema".to_string());
    }
    let req = [
        "seed",
        "starter",
        "pool_sha256",
        "runtime_sha256",
        "initial_state",
        "transitions",
        "log",
        "winner",
    ];
    if req.iter().any(|k| !obj.contains_key(*k)) {
        return Err("Replay is missing required fields".to_string());
    }
    let pool_hash = crate::adapter::pool_sha256();
    if obj.get("pool_sha256").and_then(|v| v.as_str()) != Some(pool_hash.as_str()) {
        return Err("Replay card pool does not match this installation".to_string());
    }
    if obj.get("runtime_sha256") != Some(&crate::adapter::runtime_hashes()) {
        return Err("Replay engine source does not match this installation".to_string());
    }
    if let Some(err_val) = obj.get("error") {
        if !err_val.is_null() {
            return Err("This is a diagnostic error record, not a completed replay".to_string());
        }
    }
    let seed_i32 = obj
        .get("seed")
        .and_then(as_strict_i32)
        .ok_or_else(|| "Invalid replay seed".to_string())?;
    if seed_i32 < 0 {
        return Err("Invalid replay seed".to_string());
    }
    let seed = seed_i32 as u64;
    let starter = obj
        .get("starter")
        .and_then(as_strict_i32)
        .ok_or_else(|| "Invalid replay starter".to_string())?;
    if starter != 0 && starter != 1 {
        return Err("Invalid replay starter".to_string());
    }
    let winner = obj
        .get("winner")
        .and_then(as_strict_i32)
        .ok_or_else(|| "Invalid replay winner".to_string())?;
    if winner != 0 && winner != 1 {
        return Err("Invalid replay winner".to_string());
    }
    let seeded = crate::adapter::stock_game(seed)?;
    let seeded_snap = snapshot(&seeded)?;
    if obj.get("initial_state") != Some(&seeded_snap) {
        return Err("Replay initial state does not match its seeded stock setup".to_string());
    }
    let transitions = obj.get("transitions").unwrap();
    let trans_arr = transitions.as_array().ok_or_else(|| {
        "A completed replay requires commands from the start of the match".to_string()
    })?;
    if trans_arr.is_empty() {
        return Err("A completed replay requires commands from the start of the match".to_string());
    }
    let allowed_cmds = ["start", "begin_sequence", "place", "close", "end_turn"];
    for t in trans_arr {
        let cmd = t
            .as_object()
            .and_then(|o| o.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !allowed_cmds.contains(&cmd) {
            return Err(
                "Replay contains a command that is not emitted by the local game".to_string(),
            );
        }
    }
    let first_start = &trans_arr[0];
    let first_cmd = first_start.get("command").and_then(|v| v.as_str());
    let first_selected = first_start.get("selected");
    if first_cmd != Some("start")
        || first_selected != Some(&json!({"args": [starter], "kwargs": {}}))
    {
        return Err("Replay first command does not match its starter".to_string());
    }
    let specs = crate::engine::load_proposed_specs();
    let g = replay_transitions(specs, obj.get("initial_state").unwrap(), transitions)
        .map_err(|e| format!("Invalid or divergent replay: {e}"))?;
    let last_cmd = trans_arr
        .last()
        .and_then(|t| t.get("command"))
        .and_then(|v| v.as_str());
    if g.phase != "ended" || last_cmd != Some("end_turn") {
        return Err("Replay is missing completed-match cleanup".to_string());
    }
    if g.winner != Some(winner as usize) {
        return Err("Missing or mismatched completed outcome".to_string());
    }
    let actual_log = json!(g.log);
    if &actual_log != obj.get("log").unwrap() {
        return Err("Final history mismatch".to_string());
    }
    Ok(json!({
        "ok": true,
        "winner": if winner == 0 { "Alice" } else { "White Rabbit" },
        "turns": g.turn_number,
        "transitions": g.transitions.len()
    }))
}
