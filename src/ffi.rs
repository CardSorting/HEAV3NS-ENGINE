use crate::adapter::{choices_for, custom_game_for_stage, serialize_card, stock_game};
use crate::engine::{Card, DecisionProviders, Effect, Game, Illegal, Response};
use crate::policy::{card_value, AuditPolicy};
use serde_json::{json, Map, Value};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FfiRecordedDecision {
    OptionIndex(usize),
    SelectedCards(Vec<i32>),
}

#[derive(Clone)]
struct FfiDecisionProvider {
    ai: AuditPolicy,
    preferred_target: Option<i32>,
    preferred_mode: Option<String>,
    recorded: Vec<FfiRecordedDecision>,
    cursor: usize,
    interactive_prompts: bool,
    captured_prompt: Option<Value>,
}

impl FfiDecisionProvider {
    fn new(
        preferred_target: Option<i32>,
        preferred_mode: Option<String>,
        recorded: Vec<FfiRecordedDecision>,
        interactive_prompts: bool,
    ) -> Self {
        Self {
            ai: AuditPolicy::new_heuristic(["mixed", "mixed"]),
            preferred_target,
            preferred_mode,
            recorded,
            cursor: 0,
            interactive_prompts,
            captured_prompt: None,
        }
    }

    fn next_recorded(&mut self) -> Option<FfiRecordedDecision> {
        if self.cursor < self.recorded.len() {
            let d = self.recorded[self.cursor].clone();
            self.cursor += 1;
            Some(d)
        } else {
            None
        }
    }
}

impl DecisionProviders for FfiDecisionProvider {
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
        if c.owner == 0 {
            let raw_opts = choices_for(g, c, kind, ops, pending);
            if raw_opts.len() <= 1 {
                return Ok(raw_opts
                    .into_iter()
                    .next()
                    .map(|(_, m)| m)
                    .unwrap_or_default());
            }
            if let Some(FfiRecordedDecision::OptionIndex(idx)) = self.next_recorded() {
                if let Some((_, m)) = raw_opts.get(idx) {
                    return Ok(m.clone());
                }
            }
            if !self.interactive_prompts || self.cursor == 0 {
                if let Some(pref_t) = self.preferred_target {
                    let matching: Vec<&(Value, Map<String, Value>)> = raw_opts
                        .iter()
                        .filter(|(_, m)| {
                            m.get("target").and_then(|v| v.as_i64()) == Some(pref_t as i64)
                        })
                        .collect();
                    if matching.len() == 1 {
                        return Ok(matching[0].1.clone());
                    }
                }
                if let Some(ref pref_m) = self.preferred_mode {
                    let matching: Vec<&(Value, Map<String, Value>)> = raw_opts
                        .iter()
                        .filter(|(_, m)| {
                            m.get("mode").and_then(|v| v.as_str()) == Some(pref_m.as_str())
                        })
                        .collect();
                    if matching.len() == 1 {
                        return Ok(matching[0].1.clone());
                    }
                }
            }
            if self.interactive_prompts {
                let options: Vec<Value> = raw_opts
                    .iter()
                    .enumerate()
                    .map(|(idx, (v, m))| {
                        json!({
                            "index": idx,
                            "label": v.get("label").and_then(|s| s.as_str()).unwrap_or("Choose option"),
                            "target_uid": m.get("target").and_then(|t| t.as_i64()),
                            "mode": m.get("mode").and_then(|md| md.as_str())
                        })
                    })
                    .collect();
                self.captured_prompt = Some(json!({
                    "decision_id": format!("choice:{}:{}:{}", c.uid, kind, self.cursor),
                    "kind": "choice",
                    "title": format!("Choose {}: {}", c.title(), kind),
                    "options": options,
                    "required_cards": 0
                }));
                return Err(Illegal("PROMPT_REQUIRED".into()));
            }
        }
        self.ai.choose(g, c, kind, ops, pending)
    }

    fn respond(
        &mut self,
        g: &Game,
        pending: &Effect,
        is_reply: bool,
    ) -> Result<Option<Vec<Response>>, Illegal> {
        let responder = 1 - pending.source.owner;
        if responder == 0 {
            let mut valid_counters: Vec<&Card> = Vec::new();
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
                    valid_counters.push(c);
                }
            }
            if valid_counters.is_empty() {
                return Ok(None);
            }
            if let Some(FfiRecordedDecision::OptionIndex(idx)) = self.next_recorded() {
                if idx == 0 {
                    return Ok(None);
                }
                if let Some(c) = valid_counters.get(idx - 1) {
                    return Ok(Some(vec![Response::new(c.uid)]));
                }
            }
            if self.interactive_prompts {
                let mut options = Vec::with_capacity(valid_counters.len() + 1);
                options.push(json!({
                    "index": 0,
                    "label": "Pass this response window",
                    "target_uid": Value::Null,
                    "mode": "pass"
                }));
                for (i, c) in valid_counters.iter().enumerate() {
                    options.push(json!({
                        "index": i + 1,
                        "label": format!("Play {} ({} Actions)", c.title(), c.cost()),
                        "target_uid": c.uid,
                        "mode": "respond"
                    }));
                }
                self.captured_prompt = Some(json!({
                    "decision_id": format!("response:{}:{}:{}", pending.source.uid, pending.kind, self.cursor),
                    "kind": "response",
                    "title": format!("Respond to {}: {}", pending.source.title(), pending.kind),
                    "options": options,
                    "required_cards": 0
                }));
                return Err(Illegal("PROMPT_REQUIRED".into()));
            }
        }
        self.ai.respond(g, pending, is_reply)
    }

    fn pick(&mut self, g: &Game, owner: usize, cards: &[Card]) -> Result<i32, Illegal> {
        if owner == 0 && cards.len() > 1 {
            if let Some(dec) = self.next_recorded() {
                match dec {
                    FfiRecordedDecision::OptionIndex(idx) => {
                        if let Some(c) = cards.get(idx) {
                            return Ok(c.uid);
                        }
                    }
                    FfiRecordedDecision::SelectedCards(uids) => {
                        if let Some(&first) = uids.first() {
                            if cards.iter().any(|c| c.uid == first) {
                                return Ok(first);
                            }
                        }
                    }
                }
            }
            if !self.interactive_prompts || self.cursor == 0 {
                if let Some(pref_t) = self.preferred_target {
                    if cards.iter().any(|c| c.uid == pref_t) {
                        return Ok(pref_t);
                    }
                }
            }
            if self.interactive_prompts {
                let options: Vec<Value> = cards
                    .iter()
                    .enumerate()
                    .map(|(idx, c)| {
                        json!({
                            "index": idx,
                            "label": format!("Pick {} (Cost {})", c.title(), c.cost()),
                            "target_uid": c.uid,
                            "mode": "pick"
                        })
                    })
                    .collect();
                self.captured_prompt = Some(json!({
                    "decision_id": format!("pick:{}", self.cursor),
                    "kind": "pick",
                    "title": "Look at Revealed Deck Cards - Choose One",
                    "options": options,
                    "required_cards": 1
                }));
                return Err(Illegal("PROMPT_REQUIRED".into()));
            }
        }
        self.ai.pick(g, owner, cards)
    }

    fn discard(&mut self, g: &Game, owner: usize, n: usize) -> Result<Vec<i32>, Illegal> {
        let hand = &g.p[owner].hand;
        let count = n.min(hand.len());
        if owner == 0 && count > 0 {
            if let Some(dec) = self.next_recorded() {
                match dec {
                    FfiRecordedDecision::SelectedCards(uids) if uids.len() == count => {
                        let mut uniq = uids.clone();
                        uniq.sort_unstable();
                        uniq.dedup();
                        if uniq.len() == count
                            && uids.iter().all(|u| hand.iter().any(|c| c.uid == *u))
                        {
                            return Ok(uids);
                        }
                    }
                    FfiRecordedDecision::OptionIndex(idx) => {
                        if count == 1 {
                            if let Some(c) = hand.get(idx) {
                                return Ok(vec![c.uid]);
                            }
                        } else if hand.len() >= count {
                            let mut picked = Vec::with_capacity(count);
                            for offset in 0..count {
                                picked.push(hand[(idx + offset) % hand.len()].uid);
                            }
                            return Ok(picked);
                        }
                    }
                    _ => {}
                }
            }
            if self.interactive_prompts && hand.len() > count {
                let options: Vec<Value> = hand
                    .iter()
                    .enumerate()
                    .map(|(idx, c)| {
                        json!({
                            "index": idx,
                            "label": format!("Discard {} (Cost {})", c.title(), c.cost()),
                            "target_uid": c.uid,
                            "mode": "discard"
                        })
                    })
                    .collect();
                self.captured_prompt = Some(json!({
                    "decision_id": format!("discard:{}:{}", count, self.cursor),
                    "kind": "discard",
                    "title": format!("Select {} Card(s) to Discard", count),
                    "options": options,
                    "required_cards": count
                }));
                return Err(Illegal("PROMPT_REQUIRED".into()));
            }
        }
        let mut sorted = hand.clone();
        sorted.sort_by(|a, b| {
            card_value(a)
                .partial_cmp(&card_value(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.uid.cmp(&b.uid))
        });
        Ok(sorted.into_iter().take(count).map(|c| c.uid).collect())
    }
}

/// Opaque native handle exposed over the C ABI to Unity (`Assets/Scripts/Bridge/RustEngineBridge.cs`).
pub struct NativeMatchHandle {
    pub game: Game,
    pub player_names: [String; 2],
    pub seed: u64,
    pub starter_seat: usize,
    pub revision: u64,
    pub generation: String,
    pub log_cursor: usize,
    pub pending_visual_events: Vec<Value>,
    pub interactive_prompts: bool,
    pub preferred_target: Option<i32>,
    pub preferred_mode: Option<String>,
    pub recorded_decisions: Vec<FfiRecordedDecision>,
    pub active_prompt: Option<Value>,
    pub pending_command_json: Option<Value>,
}

impl NativeMatchHandle {
    pub fn drain_new_log_events(&mut self) {
        while self.log_cursor < self.game.log.len() {
            let ev = &self.game.log[self.log_cursor];
            self.log_cursor += 1;
            let kind = ev.first().and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "start" => {
                    let seat = ev.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let turn = self.game.turn_number.max(1);
                    self.pending_visual_events.push(json!({
                        "eventType": "TurnStarted",
                        "seat": seat,
                        "turnNumber": turn,
                        "cardUid": 0,
                        "cardSpecId": "",
                        "title": "Turn Started",
                        "cost": 0,
                        "cardType": "",
                        "slotIndex": 0,
                        "targetUid": 0,
                        "amount": 0,
                        "remaining": self.game.p[seat].actions,
                        "isShieldBreak": false,
                        "triggerText": "",
                        "reason": "",
                        "text": format!("Turn {} Started", turn),
                        "subtext": format!("{}'s Turn", self.player_names.get(seat).map(String::as_str).unwrap_or("Player")),
                        "isBlocking": true,
                        "duration": 0.55
                    }));
                }
                "draw" | "select" => {
                    let seat = if kind == "draw" {
                        ev.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize
                    } else {
                        ev.get(1)
                            .and_then(|v| v.as_i64())
                            .and_then(|u| self.game.cards.get(&(u as i32)))
                            .map(|c| c.owner)
                            .unwrap_or(0)
                    };
                    let uid = if kind == "draw" {
                        ev.get(2).and_then(|v| v.as_i64()).map(|u| u as i32)
                    } else {
                        ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32)
                    };
                    let (spec_id, title) = if seat == 0 {
                        uid.and_then(|u| self.game.cards.get(&u))
                            .map(|c| (c.id().to_string(), c.title().to_string()))
                            .unwrap_or_else(|| ("".to_string(), "Card".to_string()))
                    } else {
                        ("".to_string(), "Hidden Card".to_string())
                    };
                    self.pending_visual_events.push(json!({
                        "eventType": "CardDrawn",
                        "seat": seat,
                        "turnNumber": self.game.turn_number,
                        "cardUid": if seat == 0 { uid.unwrap_or(0) } else { 0 },
                        "cardSpecId": spec_id,
                        "title": title,
                        "cost": 0,
                        "cardType": "",
                        "slotIndex": 0,
                        "targetUid": 0,
                        "amount": 1,
                        "remaining": self.game.p[seat].deck.len(),
                        "isShieldBreak": false,
                        "triggerText": "",
                        "reason": "",
                        "text": format!("{} drew a card", self.player_names.get(seat).map(String::as_str).unwrap_or("Player")),
                        "subtext": "",
                        "isBlocking": true,
                        "duration": 0.22
                    }));
                }
                "place" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "ManaSpent",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": 0,
                                "amount": c.cost(),
                                "remaining": self.game.p[c.owner].actions,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": "",
                                "text": format!("Spent {} Actions", c.cost()),
                                "subtext": "",
                                "isBlocking": false,
                                "duration": 0.25
                            }));
                            let is_char = c.card_type() == "Character";
                            let slot_index = if is_char {
                                self.game.p[c.owner]
                                    .field
                                    .iter()
                                    .position(|fc| fc.uid == uid)
                                    .unwrap_or(0)
                            } else {
                                0
                            };
                            self.pending_visual_events.push(json!({
                                "eventType": if is_char { "UnitSummoned" } else { "SpellCast" },
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": slot_index,
                                "targetUid": self.preferred_target.unwrap_or(0),
                                "amount": 0,
                                "remaining": self.game.p[c.owner].actions,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": "",
                                "text": c.title(),
                                "subtext": c.card_type(),
                                "isBlocking": true,
                                "duration": if is_char { 0.38 } else { 0.45 }
                            }));
                        }
                    }
                }
                "counter" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        let target_uid = ev.get(2).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "SpellCast",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": target_uid,
                                "amount": 0,
                                "remaining": self.game.p[c.owner].actions,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": "counter",
                                "text": format!("Counter: {}", c.title()),
                                "subtext": c.card_type(),
                                "isBlocking": true,
                                "duration": 0.45
                            }));
                        }
                    }
                }
                "reveal" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "DamageDealt",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": uid,
                                "amount": 1,
                                "remaining": self.game.p[c.owner].shields.len(),
                                "isShieldBreak": true,
                                "triggerText": c.shield_trigger().unwrap_or(""),
                                "reason": "shield_break",
                                "text": "Shield Broken!",
                                "subtext": c.title(),
                                "isBlocking": true,
                                "duration": 0.42
                            }));
                            self.pending_visual_events.push(json!({
                                "eventType": "ShieldRevealed",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": uid,
                                "amount": 1,
                                "remaining": self.game.p[c.owner].shields.len(),
                                "isShieldBreak": true,
                                "triggerText": c.shield_trigger().unwrap_or(""),
                                "reason": "",
                                "text": format!("Shield Revealed: {}", c.title()),
                                "subtext": c.shield_trigger().unwrap_or(""),
                                "isBlocking": true,
                                "duration": 0.50
                            }));
                        }
                    }
                }
                "return" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "UnitBounced",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": uid,
                                "amount": 0,
                                "remaining": 0,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": "bounce",
                                "text": format!("Returned {}", c.title()),
                                "subtext": "",
                                "isBlocking": true,
                                "duration": 0.32
                            }));
                        }
                    }
                }
                "recover" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "CardRecovered",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": uid,
                                "amount": 0,
                                "remaining": 0,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": "recover",
                                "text": format!("Recovered {}", c.title()),
                                "subtext": "",
                                "isBlocking": false,
                                "duration": 0.30
                            }));
                        }
                    }
                }
                "sequence_discard" | "discard" | "end_discard" | "shield_discard" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "UnitDestroyed",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": uid,
                                "amount": 0,
                                "remaining": 0,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": kind,
                                "text": format!("Discarded {}", c.title()),
                                "subtext": kind,
                                "isBlocking": false,
                                "duration": 0.35
                            }));
                        }
                    }
                }
                "cancel" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        let k_str = ev.get(2).and_then(|v| v.as_str()).unwrap_or("effect");
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "EffectCancelled",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": uid,
                                "amount": 0,
                                "remaining": 0,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": k_str,
                                "text": format!("Cancelled {}: {}", c.title(), k_str),
                                "subtext": "",
                                "isBlocking": true,
                                "duration": 0.40
                            }));
                        }
                    }
                }
                "combo_lost_support" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            self.pending_visual_events.push(json!({
                                "eventType": "ComboLostSupport",
                                "seat": c.owner,
                                "turnNumber": self.game.turn_number,
                                "cardUid": uid,
                                "cardSpecId": c.id(),
                                "title": c.title(),
                                "cost": c.cost(),
                                "cardType": c.card_type(),
                                "slotIndex": 0,
                                "targetUid": uid,
                                "amount": 0,
                                "remaining": 0,
                                "isShieldBreak": false,
                                "triggerText": "",
                                "reason": "combo_lost_support",
                                "text": format!("Combo lost support: {}", c.title()),
                                "subtext": "",
                                "isBlocking": false,
                                "duration": 0.35
                            }));
                        }
                    }
                }
                "win" => {
                    let seat = ev.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    self.pending_visual_events.push(json!({
                        "eventType": "MatchWon",
                        "seat": seat,
                        "turnNumber": self.game.turn_number,
                        "cardUid": 0,
                        "cardSpecId": "",
                        "title": "Match Won",
                        "cost": 0,
                        "cardType": "",
                        "slotIndex": 0,
                        "targetUid": 0,
                        "amount": 0,
                        "remaining": 0,
                        "isShieldBreak": false,
                        "triggerText": "",
                        "reason": "win",
                        "text": format!("{} Wins!", self.player_names.get(seat).map(String::as_str).unwrap_or("Player")),
                        "subtext": "",
                        "isBlocking": true,
                        "duration": 0.80
                    }));
                }
                _ => {}
            }
        }
    }

    fn run_ai_turn(&mut self, provider: &mut FfiDecisionProvider) -> Result<(), Illegal> {
        if self.game.active != Some(1) {
            self.game.start_with_providers(1, provider)?;
        }
        while self.game.winner.is_none() {
            if self.game.phase == "placement" && !self.game.sequence.is_empty() {
                let tr_len = self.game.transitions.len();
                match self.game.close_with_providers(provider) {
                    Ok(()) => {
                        self.recorded_decisions.clear();
                        provider.recorded.clear();
                        provider.cursor = 0;
                        continue;
                    }
                    Err(e) => {
                        if e.0 == "PROMPT_REQUIRED" {
                            self.game.transitions.truncate(tr_len);
                        }
                        return Err(e);
                    }
                }
            }
            let chosen = provider.ai.sequence(&self.game, 1);
            if chosen.is_empty() {
                if self.game.phase == "placement" {
                    let tr_len = self.game.transitions.len();
                    if let Err(e) = self.game.close_with_providers(provider) {
                        if e.0 == "PROMPT_REQUIRED" {
                            self.game.transitions.truncate(tr_len);
                        }
                        return Err(e);
                    }
                    self.recorded_decisions.clear();
                    provider.recorded.clear();
                    provider.cursor = 0;
                }
                break;
            }
            if self.game.phase == "resolved" {
                self.game.begin_sequence_with_providers(provider)?;
            }
            for c in chosen {
                self.game.place_with_providers(c.uid, provider)?;
            }
            let tr_len = self.game.transitions.len();
            match self.game.close_with_providers(provider) {
                Ok(()) => {
                    self.recorded_decisions.clear();
                    provider.recorded.clear();
                    provider.cursor = 0;
                }
                Err(e) => {
                    if e.0 == "PROMPT_REQUIRED" {
                        self.game.transitions.truncate(tr_len);
                    }
                    return Err(e);
                }
            }
        }
        if self.game.phase != "ended" {
            let excess = self.game.p[1].hand.len().saturating_sub(6);
            let discard = if self.game.winner.is_some() || excess == 0 {
                Vec::new()
            } else {
                provider.ai.discard(&self.game, 1, excess)?
            };
            self.game.end_turn_with_providers(&discard, provider)?;
        }
        if self.game.winner.is_none() && self.game.active != Some(0) {
            self.game.start_with_providers(0, provider)?;
        }
        Ok(())
    }

    pub fn execute_command_value(&mut self, parsed: Value) -> Value {
        let Some(cmd_name) = parsed.get("command").and_then(|v| v.as_str()) else {
            return json!({
                "status": "rejected",
                "reason": "missing_command_field",
                "revision": self.revision
            });
        };

        if let Some(interactive) = parsed.get("interactive").and_then(|v| v.as_bool()) {
            self.interactive_prompts = interactive;
        }
        if let Some(t) = parsed.get("target_uid").and_then(|v| v.as_i64()) {
            self.preferred_target = Some(t as i32);
        }
        if let Some(m) = parsed.get("mode").and_then(|v| v.as_str()) {
            self.preferred_mode = Some(m.to_string());
        }

        if cmd_name == "submit_decision" || cmd_name == "choose" || cmd_name == "select" || cmd_name == "counter" || cmd_name == "discard" {
            let option_index = parsed.get("option_index").and_then(|v| v.as_u64()).map(|n| n as usize);
            let selected_uids: Vec<i32> = parsed
                .get("selected_uids")
                .or_else(|| parsed.get("discard"))
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_i64().map(|n| n as i32)).collect())
                .unwrap_or_default();

            let dec = if !selected_uids.is_empty() {
                FfiRecordedDecision::SelectedCards(selected_uids)
            } else if let Some(idx) = option_index {
                FfiRecordedDecision::OptionIndex(idx)
            } else {
                FfiRecordedDecision::OptionIndex(0)
            };
            self.recorded_decisions.push(dec);
            self.active_prompt = None;

            if let Some(pending_cmd) = self.pending_command_json.take() {
                return self.execute_command_value(pending_cmd);
            }
            self.revision += 1;
            self.drain_new_log_events();
            return json!({
                "status": "ok",
                "revision": self.revision,
                "phase": self.game.phase,
                "turn_number": self.game.turn_number
            });
        }

        let mut provider = FfiDecisionProvider::new(
            self.preferred_target,
            self.preferred_mode.clone(),
            self.recorded_decisions.clone(),
            self.interactive_prompts,
        );

        let tr_len = self.game.transitions.len();
        let exec_result: Result<(), Illegal> = match cmd_name {
            "start" => {
                let player = parsed.get("player").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                self.game.start_with_providers(player, &provider)
            }
            "begin_sequence" => self.game.begin_sequence_with_providers(&provider),
            "place" | "play_card" => {
                let uid = parsed
                    .get("uid")
                    .or_else(|| parsed.get("card_id"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                if self.game.phase == "resolved" {
                    if let Err(e) = self.game.begin_sequence_with_providers(&provider) {
                        Err(e)
                    } else {
                        self.game.place_with_providers(uid, &provider)
                    }
                } else {
                    self.game.place_with_providers(uid, &provider)
                }
            }
            "close" | "resolve_sequence" => self.game.close_with_providers(&mut provider),
            "end_turn" => {
                let explicit_discard: Option<Vec<i32>> = parsed
                    .get("discard")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|x| x.as_i64().map(|n| n as i32)).collect());

                let mut res = Ok(());
                if self.game.active == Some(0) {
                    if self.game.phase == "placement" {
                        res = self.game.close_with_providers(&mut provider);
                        if res.is_ok() {
                            self.recorded_decisions.clear();
                            provider.recorded.clear();
                            provider.cursor = 0;
                        }
                    }
                    if res.is_ok() && self.game.winner.is_none() && self.game.phase != "ended" {
                        let excess = self.game.p[0].hand.len().saturating_sub(6);
                        let discard_vec = if let Some(d) = explicit_discard {
                            Ok(d)
                        } else if excess > 0 {
                            provider.discard(&self.game, 0, excess)
                        } else {
                            Ok(Vec::new())
                        };
                        match discard_vec {
                            Ok(discards) => {
                                res = self.game.end_turn_with_providers(&discards, &provider);
                                if res.is_ok() {
                                    self.recorded_decisions.clear();
                                    provider.recorded.clear();
                                    provider.cursor = 0;
                                }
                            }
                            Err(e) => res = Err(e),
                        }
                    }
                } else {
                    let discards = explicit_discard.unwrap_or_default();
                    res = self.game.end_turn_with_providers(&discards, &provider);
                }

                if res.is_ok()
                    && parsed
                        .get("run_ai")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    && self.game.winner.is_none()
                {
                    res = self.run_ai_turn(&mut provider);
                }
                res
            }
            other => Err(Illegal(format!("Unknown command: {other}"))),
        };

        self.drain_new_log_events();

        match exec_result {
            Ok(()) => {
                self.revision += 1;
                self.active_prompt = None;
                self.pending_command_json = None;
                self.recorded_decisions.clear();
                json!({
                    "status": "ok",
                    "revision": self.revision,
                    "phase": self.game.phase,
                    "turn_number": self.game.turn_number,
                    "winner": self.game.winner
                })
            }
            Err(illegal) if illegal.0 == "PROMPT_REQUIRED" => {
                self.game.transitions.truncate(tr_len);
                self.active_prompt = provider.captured_prompt.clone();
                self.pending_command_json = Some(parsed);
                json!({
                    "status": "prompt_required",
                    "revision": self.revision,
                    "phase": self.game.phase,
                    "turn_number": self.game.turn_number,
                    "prompt": self.active_prompt
                })
            }
            Err(illegal) => json!({
                "status": "rejected",
                "reason": illegal.0,
                "revision": self.revision
            }),
        }
    }
}

fn to_c_string_ptr(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Creates a new seeded Wonderland match instance and returns an opaque handle pointer.
#[no_mangle]
pub extern "C" fn heav3ns_engine_create(seed: u64, starter_seat: u32) -> *mut NativeMatchHandle {
    let Ok(game) = stock_game(seed) else {
        return ptr::null_mut();
    };
    create_handle(game, seed, starter_seat, ["Alice".into(), "White Rabbit".into()])
}

/// Starts a match with a validated 30-card deck encoded as a JSON array of card IDs.
/// Returns null if the JSON, deck, or seed is invalid.
///
/// # Safety
/// `deck_json` must point to a valid NUL-terminated UTF-8 string for this call.
#[no_mangle]
pub unsafe extern "C" fn heav3ns_engine_create_with_deck(seed: u64, starter_seat: u32, deck_json: *const c_char) -> *mut NativeMatchHandle {
    heav3ns_engine_create_with_scenario(seed, starter_seat, deck_json, 0)
}

/// # Safety
/// `deck_json` must point to a valid NUL-terminated UTF-8 string for this call.
#[no_mangle]
pub unsafe extern "C" fn heav3ns_engine_create_with_scenario(seed: u64, starter_seat: u32, deck_json: *const c_char, stage: u32) -> *mut NativeMatchHandle {
    if deck_json.is_null() { return ptr::null_mut(); }
    let Ok(text) = CStr::from_ptr(deck_json).to_str() else { return ptr::null_mut(); };
    let Ok(ids) = serde_json::from_str::<Vec<String>>(text) else { return ptr::null_mut(); };
    let Ok((game, names)) = custom_game_for_stage(seed, &ids, stage) else { return ptr::null_mut(); };
    create_handle(game, seed, starter_seat, names)
}

fn create_handle(mut game: Game, seed: u64, starter_seat: u32, player_names: [String; 2]) -> *mut NativeMatchHandle {
    let starter = (starter_seat as usize) % 2;
    game.trace_transitions = true;
    let provider = FfiDecisionProvider::new(None, None, Vec::new(), false);
    let _ = game.start_with_providers(starter, &provider);

    let mut handle = Box::new(NativeMatchHandle {
        game,
        player_names,
        seed,
        starter_seat: starter,
        revision: 1,
        generation: crate::adapter::next_generation_hex(),
        log_cursor: 0,
        pending_visual_events: Vec::new(),
        interactive_prompts: false,
        preferred_target: None,
        preferred_mode: None,
        recorded_decisions: Vec::new(),
        active_prompt: None,
        pending_command_json: None,
    });
    handle.drain_new_log_events();
    Box::into_raw(handle)
}

/// Destroys a previously created `NativeMatchHandle` and frees its memory.
///
/// # Safety
/// `handle` must be either null or a valid pointer previously returned by `heav3ns_engine_create`
/// that has not yet been destroyed.
#[no_mangle]
pub unsafe extern "C" fn heav3ns_engine_destroy(handle: *mut NativeMatchHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Submits a serialized JSON command to the native Rust engine and returns a UTF-8 JSON response string.
///
/// # Safety
/// `handle` must be either null or a valid `NativeMatchHandle` pointer, and `command_json` must be either
/// null or a valid null-terminated C string pointer.
#[no_mangle]
pub unsafe extern "C" fn heav3ns_engine_submit_command_json(
    handle: *mut NativeMatchHandle,
    command_json: *const c_char,
) -> *mut c_char {
    if handle.is_null() || command_json.is_null() {
        return to_c_string_ptr(
            json!({ "status": "error", "reason": "null_pointer" }).to_string(),
        );
    }

    let handle_ref = &mut *handle;
    let raw_cmd = match CStr::from_ptr(command_json).to_str() {
        Ok(s) => s,
        Err(_) => {
            return to_c_string_ptr(
                json!({ "status": "error", "reason": "invalid_utf8" }).to_string(),
            );
        }
    };

    let parsed: Value = match serde_json::from_str(raw_cmd) {
        Ok(v) => v,
        Err(e) => {
            return to_c_string_ptr(
                json!({ "status": "error", "reason": format!("invalid_json: {}", e) }).to_string(),
            );
        }
    };

    let resp = handle_ref.execute_command_value(parsed);
    to_c_string_ptr(resp.to_string())
}

/// Drains pending `VisualEvent` records emitted by the Rust engine as a UTF-8 JSON object (`{"events":[...]}`).
///
/// # Safety
/// `handle` must be either null or a valid `NativeMatchHandle` pointer.
#[no_mangle]
pub unsafe extern "C" fn heav3ns_engine_drain_visual_events_json(
    handle: *mut NativeMatchHandle,
) -> *mut c_char {
    if handle.is_null() {
        return to_c_string_ptr(json!({ "events": [] }).to_string());
    }
    let handle_ref = &mut *handle;
    handle_ref.drain_new_log_events();
    let drained: Vec<Value> = handle_ref.pending_visual_events.drain(..).collect();
    to_c_string_ptr(json!({ "events": drained }).to_string())
}

/// Returns the current redacted match state snapshot as a UTF-8 JSON string.
///
/// # Safety
/// `handle` must be either null or a valid `NativeMatchHandle` pointer.
#[no_mangle]
pub unsafe extern "C" fn heav3ns_engine_get_board_state_json(
    handle: *mut NativeMatchHandle,
) -> *mut c_char {
    if handle.is_null() {
        return to_c_string_ptr(json!({ "status": "error", "reason": "null_handle" }).to_string());
    }
    let handle_ref = &*handle;
    let g = &handle_ref.game;
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
            "name": handle_ref.player_names[owner],
            "actions": p.actions,
            "hand": hand_json,
            "hand_count": p.hand.len(),
            "deck_count": p.deck.len(),
            "shields": shields_json,
            "field": field_json,
            "discard": discard_json
        }));
    }
    let sequence_json: Vec<Value> = g.sequence.iter().map(serialize_card).collect();
    let state = json!({
        "status": if handle_ref.active_prompt.is_some() { "prompt_required" } else { "ready" },
        "seed": handle_ref.seed,
        "revision": handle_ref.revision,
        "generation": handle_ref.generation,
        "turn_number": g.turn_number,
        "active": g.active,
        "phase": g.phase,
        "winner": g.winner,
        "sequence": sequence_json,
        "prompt": handle_ref.active_prompt,
        "players": players
    });
    to_c_string_ptr(state.to_string())
}

/// Frees a C string previously allocated and returned by `heav3ns_engine_*`.
///
/// # Safety
/// `ptr` must be either null or a pointer previously returned by one of the `heav3ns_engine_*` functions
/// that has not yet been freed.
#[no_mangle]
pub unsafe extern "C" fn heav3ns_engine_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn consume_c_str(ptr: *mut c_char) -> Value {
        assert!(!ptr.is_null());
        let s = CStr::from_ptr(ptr).to_str().unwrap().to_string();
        heav3ns_engine_free_string(ptr);
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn test_ffi_lifecycle_real_events_and_invalid_command_rejection() {
        unsafe {
            let handle = heav3ns_engine_create(42, 0);
            assert!(!handle.is_null());

            // 1. Drain initial TurnStarted + CardDrawn events from start()
            let initial_events = consume_c_str(heav3ns_engine_drain_visual_events_json(handle));
            let ev_arr = initial_events["events"].as_array().unwrap();
            assert!(!ev_arr.is_empty());
            assert_eq!(ev_arr[0]["eventType"], "TurnStarted");

            // 2. Verify unknown commands are rejected instead of returning status: ok
            let bad_cmd = CString::new(r#"{"command":"unknown_typo"}"#).unwrap();
            let bad_resp = consume_c_str(heav3ns_engine_submit_command_json(handle, bad_cmd.as_ptr()));
            assert_eq!(bad_resp["status"], "rejected");

            // 3. Get board state and play a valid card from Alice's hand
            let state = consume_c_str(heav3ns_engine_get_board_state_json(handle));
            assert_eq!(state["status"], "ready");
            let hand = state["players"][0]["hand"].as_array().unwrap();
            let playable_uid = hand
                .iter()
                .find(|c| c["type"] != "Counter" && c["cost"].as_i64().unwrap_or(99) <= 6)
                .unwrap()["uid"]
                .as_i64()
                .unwrap();

            let place_cmd = CString::new(format!(r#"{{"command":"place","uid":{playable_uid}}}"#)).unwrap();
            let place_resp = consume_c_str(heav3ns_engine_submit_command_json(handle, place_cmd.as_ptr()));
            assert_eq!(place_resp["status"], "ok");

            // 4. Drain events after place and verify real ManaSpent + UnitSummoned/SpellCast events
            let place_events = consume_c_str(heav3ns_engine_drain_visual_events_json(handle));
            let place_ev_arr = place_events["events"].as_array().unwrap();
            assert!(place_ev_arr.iter().any(|e| e["eventType"] == "ManaSpent"));
            assert!(place_ev_arr
                .iter()
                .any(|e| e["eventType"] == "UnitSummoned" || e["eventType"] == "SpellCast"));

            // 5. Close sequence and end turn with AI response
            let end_cmd = CString::new(r#"{"command":"end_turn","run_ai":true}"#).unwrap();
            let end_resp = consume_c_str(heav3ns_engine_submit_command_json(handle, end_cmd.as_ptr()));
            assert_eq!(end_resp["status"], "ok");

            heav3ns_engine_destroy(handle);
        }
    }
}


