use crate::engine::{
    as_strict_i32, decode_mode, is_bounce, Card, DecisionProviders, Effect, Game, Illegal, Response,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

pub fn policy_weights(name: &str) -> (f64, f64, i32) {
    match name {
        "offense" => (6.0, 0.65, 0),
        "mixed" => (4.5, 1.0, 1),
        "control" => (3.5, 1.5, 2),
        "resource" => (1.5, 2.0, 2),
        _ => (4.5, 1.0, 1),
    }
}

pub fn unknown_spec() -> Value {
    json!({
        "id": "UNKNOWN",
        "title": "Unseen card",
        "type": "Action",
        "cost": 6,
        "ops": [],
        "entry_ops": []
    })
}

pub fn nominal_pressure(ops: &[Value]) -> i32 {
    let mut total = 0;
    for op in ops {
        let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if k == "break" || k == "break_finisher" {
            total += op.get("n").and_then(as_strict_i32).unwrap_or(0);
        } else if k == "choice" {
            let max_branch = op
                .get("choices")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.as_str())
                        .filter_map(|ms| decode_mode(ms).ok())
                        .map(|d| nominal_pressure(&[d]))
                        .max()
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            total += max_branch;
        }
    }
    total
}

pub fn nominal_resources(ops: &[Value]) -> f64 {
    let mut total = 0.0;
    for op in ops {
        let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if k == "choice" {
            let max_branch = op
                .get("choices")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.as_str())
                        .filter_map(|ms| decode_mode(ms).ok())
                        .map(|d| nominal_resources(&[d]))
                        .fold(0.0_f64, f64::max)
                })
                .unwrap_or(0.0);
            total += max_branch;
        } else if k == "draw" {
            total += op.get("n").and_then(as_strict_i32).unwrap_or(0) as f64;
        } else if k == "select" {
            total += 1.3;
        } else if k.starts_with("recover") {
            total += 1.2;
        } else if k == "discard_enemy" {
            total += 0.8 * (op.get("n").and_then(as_strict_i32).unwrap_or(1) as f64);
        } else if k.starts_with("bounce") {
            total += 0.7;
        }
    }
    total
}

pub fn all_card_ops(c: &Card) -> Vec<Value> {
    let mut ops = Vec::new();
    if let Some(arr) = c.spec.get("ops").and_then(|v| v.as_array()) {
        ops.extend(arr.clone());
    }
    if let Some(arr) = c.spec.get("entry_ops").and_then(|v| v.as_array()) {
        ops.extend(arr.clone());
    }
    if let Some(arr) = c
        .spec
        .get("combo")
        .and_then(|v| v.get("ops"))
        .and_then(|v| v.as_array())
    {
        ops.extend(arr.clone());
    }
    ops
}

pub fn card_value(c: &Card) -> f64 {
    let ops = all_card_ops(c);
    let mut rank = 0.0;
    for op in &ops {
        let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if k == "choice" {
            let max_b = op
                .get("choices")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.as_str())
                        .filter_map(|ms| decode_mode(ms).ok())
                        .map(|o| {
                            (nominal_pressure(&[o.clone()]) as f64) * 2.0
                                + nominal_resources(&[o]) * 0.6
                        })
                        .fold(0.0_f64, f64::max)
                })
                .unwrap_or(0.0);
            rank += max_b;
        } else {
            rank += (nominal_pressure(std::slice::from_ref(op)) as f64) * 2.0
                + nominal_resources(std::slice::from_ref(op)) * 0.6;
        }
    }
    let counter_bonus = if c.card_type() == "Counter" || c.response_play() {
        1.5
    } else {
        0.0
    };
    let cost = c.cost().max(1) as f64;
    rank + counter_bonus + 1.0 / cost
}

pub fn field_target_value(c: &Card) -> f64 {
    let co_ops = c
        .spec
        .get("combo")
        .and_then(|v| v.get("ops"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let entry_len = c
        .spec
        .get("entry_ops")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0) as f64;
    (nominal_pressure(&co_ops) as f64) * 3.0 + entry_len + 0.1 * (c.cost() as f64)
}

/// Creates a redacted public clone of `g` from `owner`'s perspective, replacing concealed cards with `UNKNOWN`.
pub fn public_clone(g: &Game, owner: usize) -> Game {
    let mut h = Game::new_sim(Some(g.specs_list.clone()), g.opening_protection)
        .expect("public_clone valid specs");
    h.turn_number = g.turn_number;
    h.phase = g.phase.clone();
    h.active = g.active;
    h.winner = g.winner;
    let unk = unknown_spec();
    for who in 0..2 {
        h.p[who].actions = g.p[who].actions;
        for zone in ["hand", "deck", "field", "shields", "discard"] {
            for c in g.p[who].zone(zone) {
                let hidden =
                    zone == "deck" || zone == "shields" || (zone == "hand" && who != owner);
                let mut q = Card::new(
                    c.uid,
                    c.owner,
                    if hidden { unk.clone() } else { c.spec.clone() },
                );
                q.cancelled = c.cancelled;
                q.combo_used = c.combo_used;
                h.cards.insert(q.uid, q.clone());
                h.p[who].zone_mut(zone).push(q);
            }
        }
    }
    h.sequence = g
        .sequence
        .iter()
        .filter_map(|c| h.cards.get(&c.uid).cloned())
        .collect();
    h
}

/// Enumerates all distinct-card-spec affordable sequences in order.
pub fn affordable_sequences(hand: &[Card], budget: i32) -> Vec<Vec<Card>> {
    let mut out = Vec::new();
    fn visit(seq: Vec<Card>, remaining: Vec<Card>, left: i32, out: &mut Vec<Vec<Card>>) {
        let mut seen = HashSet::new();
        for (idx, c) in remaining.iter().enumerate() {
            if c.card_type() == "Counter" || c.cost() > left || seen.contains(c.id()) {
                continue;
            }
            seen.insert(c.id().to_string());
            let mut new_seq = seq.clone();
            new_seq.push(c.clone());
            out.push(new_seq.clone());
            let mut next_rem = remaining.clone();
            next_rem.remove(idx);
            visit(new_seq, next_rem, left - c.cost(), out);
        }
    }
    visit(Vec::new(), hand.to_vec(), budget, &mut out);
    out
}

/// Heuristic `Policy` and visible-state `AuditPolicy` implementation.
#[derive(Clone)]
pub struct AuditPolicy {
    pub names: [String; 2],
    pub metrics: BTreeMap<String, i64>,
    pub planning: bool,
    pub audit_mode: bool,
}

impl AuditPolicy {
    pub fn new([n0, n1]: [impl Into<String>; 2]) -> Self {
        Self {
            names: [n0.into(), n1.into()],
            metrics: BTreeMap::new(),
            planning: false,
            audit_mode: true,
        }
    }

    pub fn new_heuristic([n0, n1]: [impl Into<String>; 2]) -> Self {
        Self {
            names: [n0.into(), n1.into()],
            metrics: BTreeMap::new(),
            planning: false,
            audit_mode: false,
        }
    }

    pub fn recover_targets(&self, g: &Game, owner: usize, kind: &str) -> Vec<Card> {
        g.p[owner]
            .discard
            .iter()
            .filter(|t| match kind {
                "recover_character" => t.card_type() == "Character",
                "recover_action" => t.card_type() == "Action" || t.card_type() == "Counter",
                _ => true,
            })
            .cloned()
            .collect()
    }

    pub fn is_progress(&self, c: &Card) -> bool {
        nominal_pressure(&all_card_ops(c)) > 0
    }

    pub fn is_access(&self, g: &Game, c: &Card) -> bool {
        let ops = all_card_ops(c);
        let mut expanded = Vec::new();
        for o in ops {
            if o.get("kind").and_then(|v| v.as_str()) == Some("choice") {
                if let Some(arr) = o.get("choices").and_then(|v| v.as_array()) {
                    for m in arr {
                        if let Some(ms) = m.as_str() {
                            if let Ok(d) = decode_mode(ms) {
                                expanded.push(d);
                            }
                        }
                    }
                }
            } else {
                expanded.push(o);
            }
        }
        expanded.iter().any(|o| {
            let k = o.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if k == "draw" || k == "select" {
                !g.p[c.owner].deck.is_empty()
            } else if k.starts_with("recover") {
                !self.recover_targets(g, c.owner, k).is_empty()
            } else {
                false
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn op_value(
        &self,
        g: &Game,
        c: &Card,
        op: &Value,
        hand_count: Option<usize>,
        seq: Option<&[Card]>,
        index: Option<usize>,
        shields: Option<usize>,
        deck_count: Option<usize>,
        recovered: Option<&HashSet<i32>>,
        enemy_hand_count: Option<usize>,
    ) -> f64 {
        let owner = c.owner;
        let (damage_weight, draw_weight, _) = policy_weights(&self.names[owner]);
        let hc = hand_count.unwrap_or(g.p[owner].hand.len());
        let scarcity = if hc <= 2 {
            2.5
        } else if hc <= 4 {
            1.5
        } else {
            0.6
        };
        let resource = draw_weight * scarcity;
        let sh = shields.unwrap_or(g.p[1 - owner].shields.len());
        let dc = deck_count.unwrap_or(g.p[owner].deck.len());
        let ehc = enemy_hand_count.unwrap_or(g.p[1 - owner].hand.len());
        let empty_set = HashSet::new();
        let rec = recovered.unwrap_or(&empty_set);

        let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let mut n = op.get("n").and_then(as_strict_i32).unwrap_or(1) as usize;
        if k == "break" || k == "break_finisher" {
            if k == "break_finisher" {
                let threshold = op.get("threshold").and_then(as_strict_i32).unwrap_or(0) as usize;
                if sh <= threshold {
                    n = op.get("finish").and_then(as_strict_i32).unwrap_or(0) as usize;
                }
            }
            let lethal_bonus = if sh > 0 && n >= sh { 50.0 } else { 0.0 };
            return (n.min(sh) as f64) * damage_weight + lethal_bonus;
        }
        if k == "draw" {
            return (n.min(dc) as f64) * resource;
        }
        if k == "select" {
            return if dc > 0 { resource * 1.3 } else { 0.0 };
        }
        if k.starts_with("recover") {
            let targets: Vec<Card> = self
                .recover_targets(g, owner, k)
                .into_iter()
                .filter(|t| !rec.contains(&t.uid))
                .collect();
            if targets.is_empty() {
                return 0.0;
            }
            let quality = targets.iter().map(card_value).fold(0.0_f64, f64::max);
            let precision = if self.names[owner] == "resource" {
                0.25
            } else {
                0.12
            };
            return resource * (1.0 + precision * quality);
        }
        if k == "discard_enemy" {
            return (n.min(ehc) as f64) * draw_weight * 1.3;
        }
        if k.starts_with("bounce") {
            let seq_slice = seq.unwrap_or(&g.sequence);
            let idx = index.unwrap_or_else(|| {
                seq_slice
                    .iter()
                    .position(|x| x.uid == c.uid)
                    .unwrap_or(seq_slice.len())
            });
            if k.contains("enemy") {
                let mut targets: Vec<&Card> = g.p[1 - owner]
                    .field
                    .iter()
                    .filter(|t| t.card_type() == "Character")
                    .collect();
                if k == "bounce_enemy_max2" {
                    targets.retain(|t| t.cost() <= 2);
                }
                return targets
                    .into_iter()
                    .map(|t| field_target_value(t) + 1.0)
                    .fold(0.0_f64, f64::max);
            }
            let targets: Vec<&Card> = seq_slice
                .iter()
                .filter(|t| {
                    t.owner == owner
                        && t.card_type() == "Character"
                        && (k != "bounce_other" || t.uid != c.uid)
                })
                .collect();
            let mut values = Vec::new();
            for t in targets {
                let t_pos = seq_slice.iter().position(|x| x.uid == t.uid).unwrap_or(0);
                let earlier = t_pos < idx;
                if earlier {
                    values.push(resource * 0.7);
                } else {
                    let co_ops = t
                        .spec
                        .get("combo")
                        .and_then(|v| v.get("ops"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    values.push(-damage_weight * ((nominal_pressure(&co_ops) + 1) as f64));
                }
            }
            let val = if values.is_empty() {
                0.0
            } else {
                values.into_iter().fold(f64::NEG_INFINITY, f64::max)
            };
            let optional = op
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            return if optional { val.max(0.0) } else { val };
        }
        0.0
    }

    #[allow(clippy::too_many_arguments)]
    pub fn choose_mode(
        &self,
        g: &Game,
        c: &Card,
        options: &[String],
        hand_count: Option<usize>,
        seq: Option<&[Card]>,
        index: Option<usize>,
        shields: Option<usize>,
        deck_count: Option<usize>,
        recovered: Option<&HashSet<i32>>,
        enemy_hand_count: Option<usize>,
    ) -> String {
        let mut best_mode = options[0].clone();
        let mut best_score = f64::NEG_INFINITY;
        let mut best_neg_order = isize::MIN;
        for (order, mode) in options.iter().enumerate() {
            let mut ch = Map::new();
            ch.insert("mode".to_string(), json!(mode));
            let op = g
                .expanded(&[json!({"kind": "choice", "choices": options})], &ch)
                .ok()
                .and_then(|mut v| v.pop())
                .unwrap_or_else(|| json!({"kind": mode}));
            let score = self.op_value(
                g,
                c,
                &op,
                hand_count,
                seq,
                index,
                shields,
                deck_count,
                recovered,
                enemy_hand_count,
            );
            let neg_order = -(order as isize);
            if score > best_score || (score == best_score && neg_order > best_neg_order) {
                best_score = score;
                best_neg_order = neg_order;
                best_mode = mode.clone();
            }
        }
        best_mode
    }

    pub fn base_choose(
        &mut self,
        g: &Game,
        c: &Card,
        kind: &str,
        ops: &[Value],
        pending: Option<&Effect>,
    ) -> Map<String, Value> {
        let mut choice = Map::new();
        let options: Vec<String> = ops
            .iter()
            .filter(|o| o.get("kind").and_then(|v| v.as_str()) == Some("choice"))
            .filter_map(|o| o.get("choices").and_then(|v| v.as_array()))
            .flatten()
            .filter_map(|m| m.as_str().map(|s| s.to_string()))
            .collect();
        if !options.is_empty() {
            let m = self.choose_mode(g, c, &options, None, None, None, None, None, None, None);
            choice.insert("mode".to_string(), json!(m));
        }
        let expanded = g.expanded(ops, &choice).unwrap_or_default();
        for op in &expanded {
            let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let optional = op
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if optional
                && k.starts_with("bounce")
                && pending.is_none()
                && self.op_value(g, c, op, None, None, None, None, None, None, None) <= 0.0
            {
                choice.insert("mode".to_string(), json!("skip"));
                continue;
            }
            if is_bounce(k) || k == "counter_bounce_later" {
                let e = Effect::new(
                    c.clone(),
                    kind,
                    expanded.clone(),
                    choice.clone(),
                    pending.map(|p| Box::new(p.clone())),
                );
                let targets = g.bounce_candidates(&e, k);
                if !targets.is_empty() {
                    let chosen_target = if let Some(p) = pending {
                        if targets.iter().any(|t| t.uid == p.source.uid) {
                            p.source.uid
                        } else {
                            targets
                                .iter()
                                .max_by(|a, b| {
                                    field_target_value(a)
                                        .partial_cmp(&field_target_value(b))
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                        .then_with(|| b.uid.cmp(&a.uid))
                                })
                                .unwrap()
                                .uid
                        }
                    } else {
                        let current = g
                            .sequence
                            .iter()
                            .position(|x| x.uid == c.uid)
                            .unwrap_or(g.sequence.len());
                        targets
                            .iter()
                            .min_by(|a, b| {
                                let a_later = g
                                    .sequence
                                    .iter()
                                    .position(|x| x.uid == a.uid)
                                    .map(|idx| idx >= current)
                                    .unwrap_or(false);
                                let b_later = g
                                    .sequence
                                    .iter()
                                    .position(|x| x.uid == b.uid)
                                    .map(|idx| idx >= current)
                                    .unwrap_or(false);
                                a_later
                                    .cmp(&b_later)
                                    .then_with(|| {
                                        card_value(a)
                                            .partial_cmp(&card_value(b))
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    })
                                    .then_with(|| a.uid.cmp(&b.uid))
                            })
                            .unwrap()
                            .uid
                    };
                    choice.insert("target".to_string(), json!(chosen_target));
                }
            } else if k.starts_with("recover") {
                let targets = self.recover_targets(g, c.owner, k);
                if !targets.is_empty() {
                    let best_t = targets
                        .iter()
                        .max_by(|a, b| {
                            card_value(a)
                                .partial_cmp(&card_value(b))
                                .unwrap_or(std::cmp::Ordering::Equal)
                                .then_with(|| b.uid.cmp(&a.uid))
                        })
                        .unwrap()
                        .uid;
                    choice.insert("target".to_string(), json!(best_t));
                }
            }
        }
        choice
    }

    pub fn recovery_rank(&mut self, g: &Game, target: &Card) -> (i32, i32, f64, f64, f64, i32) {
        let owner = target.owner;
        if self.planning {
            let affordable = if target.cost() <= g.p[owner].actions {
                1
            } else {
                0
            };
            return (0, affordable, card_value(target), 0.0, 0.0, -target.uid);
        }
        let mut h = public_clone(g, owner);
        for who in 0..2 {
            let f = std::mem::take(&mut h.p[who].field);
            h.p[who].discard.extend(f);
        }
        h.sequence.clear();
        h.phase = "resolved".to_string();
        if let Some(q) = h.cards.get(&target.uid).cloned() {
            h.p[owner].discard.retain(|x| x.uid != q.uid);
            h.p[owner].hand.push(q);
        }
        let mut sub_policy = AuditPolicy::new(self.names.clone());
        sub_policy.planning = true;

        let mut best = 0.0_f64;
        let mut lethal = false;
        let mut progress = 0_i32;
        let seqs = affordable_sequences(&h.p[owner].hand, h.p[owner].actions);
        for (i, seq) in seqs.into_iter().enumerate() {
            if i >= 1000 {
                *self
                    .metrics
                    .entry("recovery_continuation_cap_hits".to_string())
                    .or_insert(0) += 1;
                break;
            }
            let (value, win, damage) = sub_policy.rollout_result(&h, &seq, owner);
            best = best.max(value);
            lethal = lethal || win;
            progress = progress.max(damage);
        }

        h.p[owner].actions = 6;
        let mut next_value = 0.0_f64;
        let next_seqs = affordable_sequences(&h.p[owner].hand, 6);
        for (i, seq) in next_seqs.into_iter().enumerate() {
            if i >= 1000 {
                *self
                    .metrics
                    .entry("recovery_nextturn_cap_hits".to_string())
                    .or_insert(0) += 1;
                break;
            }
            let (value, _, _) = sub_policy.rollout_result(&h, &seq, owner);
            next_value = next_value.max(value);
        }
        (
            if lethal { 1 } else { 0 },
            progress,
            best,
            next_value,
            card_value(target),
            -target.uid,
        )
    }

    pub fn rollout_result(&mut self, g: &Game, seq: &[Card], owner: usize) -> (f64, bool, i32) {
        let mut h = public_clone(g, owner);
        struct RolloutProviders {
            policy: AuditPolicy,
        }
        impl DecisionProviders for RolloutProviders {
            fn has_choice(&self) -> bool {
                true
            }
            fn has_response(&self) -> bool {
                false
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
                self.policy.choose(g, c, kind, ops, pending)
            }
            fn pick(&mut self, g: &Game, owner: usize, cards: &[Card]) -> Result<i32, Illegal> {
                self.policy.pick(g, owner, cards)
            }
            fn discard(&mut self, g: &Game, owner: usize, n: usize) -> Result<Vec<i32>, Illegal> {
                self.policy.discard(g, owner, n)
            }
        }
        let mut sub = AuditPolicy::new(self.names.clone());
        sub.planning = true;
        let mut rp = RolloutProviders { policy: sub };

        if h.phase == "resolved" {
            let _ = h.begin_sequence_with_providers(&rp);
        }
        for c in seq {
            let _ = h.place_with_providers(c.uid, &rp);
        }
        let before_hand: HashSet<i32> = h.p[owner].hand.iter().map(|c| c.uid).collect();
        let before_enemy = h.p[1 - owner].hand.len();
        let shields = h.p[1 - owner].shields.len() as i32;
        let _ = h.close_with_providers(&mut rp);
        let _ = h.invariants();

        let (damage, resource, reserve) = policy_weights(&self.names[owner]);
        let rem_hand = g.p[owner].hand.len().saturating_sub(seq.len());
        let scarcity = if rem_hand <= 2 {
            2.5
        } else if rem_hand <= 4 {
            1.5
        } else {
            0.6
        };
        let shields_after = h.p[1 - owner].shields.len() as i32;
        let broken = shields - shields_after;
        let mut score = (broken as f64) * damage;
        if h.winner == Some(owner) {
            score += 10000.0;
        }
        for c in &h.p[owner].hand {
            if !before_hand.contains(&c.uid) {
                let quality = if c.id() == "UNKNOWN" {
                    0.0
                } else {
                    let mult = if self.names[owner] == "resource" {
                        0.25
                    } else {
                        0.12
                    };
                    card_value(c) * mult
                };
                score += resource * scarcity * (1.0 + quality);
            }
        }
        let enemy_lost = before_enemy.saturating_sub(h.p[1 - owner].hand.len());
        score += (enemy_lost as f64) * resource * 1.3;
        let spent: i32 = seq.iter().map(|c| c.cost()).sum();
        score -= (spent as f64) * 0.12;
        if g.p[owner]
            .hand
            .iter()
            .any(|c| c.card_type() == "Counter" || c.response_play())
        {
            let short = (reserve - (g.p[owner].actions - spent)).max(0);
            score -= (short as f64) * 0.8;
        }
        (score, h.winner == Some(owner), broken)
    }

    pub fn effect_value(&self, g: &Game, mut e: &Effect) -> f64 {
        while e.kind == "counter" {
            if let Some(ref target_eff) = e.target_effect {
                e = target_eff;
            } else {
                break;
            }
        }
        let mut value = 0.0;
        for op in &e.ops {
            let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            match k {
                "break" => {
                    value += (op.get("n").and_then(as_strict_i32).unwrap_or(0) as f64) * 3.0;
                }
                "break_finisher" => {
                    let threshold =
                        op.get("threshold").and_then(as_strict_i32).unwrap_or(0) as usize;
                    let amt = if g.p[1 - e.source.owner].shields.len() <= threshold {
                        op.get("finish").and_then(as_strict_i32).unwrap_or(0)
                    } else {
                        op.get("n").and_then(as_strict_i32).unwrap_or(0)
                    };
                    value += (amt as f64) * 3.0;
                }
                "draw" => {
                    value += (op.get("n").and_then(as_strict_i32).unwrap_or(1) as f64) * 0.8;
                }
                "select" => {
                    value += 1.3 * 0.8;
                }
                _ if k.starts_with("recover") => {
                    if e.choice.get("target").and_then(as_strict_i32).is_some() {
                        value += 1.2;
                    }
                }
                _ if is_bounce(k) || k == "counter_bounce_later" => {
                    if let Some(t_uid) = e.choice.get("target").and_then(as_strict_i32) {
                        if let Some(target) = g.cards.get(&t_uid) {
                            value += 2.0 + field_target_value(target);
                        }
                    }
                }
                "discard_enemy" => {
                    let n = op.get("n").and_then(as_strict_i32).unwrap_or(1) as usize;
                    let enemy_h = g.p[1 - e.source.owner].hand.len();
                    value += (n.min(enemy_h) as f64) * 1.2;
                }
                _ => {}
            }
        }
        value
    }

    pub fn sequence(&mut self, g: &Game, owner: usize) -> Vec<Card> {
        if !self.audit_mode {
            return self.heuristic_sequence(g, owner);
        }
        let mut best = Vec::new();
        let mut best_score = 0.0_f64;
        let mut count = 0_i64;
        let seqs = affordable_sequences(&g.p[owner].hand, g.p[owner].actions);
        for seq in seqs {
            if count >= 50000 {
                *self
                    .metrics
                    .entry("search_cap_hits".to_string())
                    .or_insert(0) += 1;
                break;
            }
            let (score, _) = {
                let (s, w, _) = self.rollout_result(g, &seq, owner);
                (s, w)
            };
            count += 1;
            if score > best_score + 1e-9 {
                best_score = score;
                best = seq;
            }
        }
        *self
            .metrics
            .entry("enumerated_sequences".to_string())
            .or_insert(0) += count;
        best
    }

    pub fn heuristic_score(&self, g: &Game, seq: &[Card], owner: usize) -> f64 {
        let chars = seq.iter().filter(|c| c.card_type() == "Character").count() as i32;
        let mut score = 0.0;
        let mut shields = g.p[1 - owner].shields.len();
        let mut projected_hand = g.p[owner].hand.len().saturating_sub(seq.len());
        let mut projected_deck = g.p[owner].deck.len();
        let mut recovered = HashSet::new();
        let mut enemy_hand = g.p[1 - owner].hand.len();

        for (i, c) in seq.iter().enumerate() {
            let mut phases = vec![if c.card_type() == "Action" {
                c.spec
                    .get("ops")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
            } else {
                c.spec
                    .get("entry_ops")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
            }];
            if let Some(co_n) = c.combo_n() {
                if ((seq.len() - i - 1) as i32) >= co_n {
                    if let Some(co_ops) = c
                        .spec
                        .get("combo")
                        .and_then(|v| v.get("ops"))
                        .and_then(|v| v.as_array())
                    {
                        phases.push(co_ops.clone());
                    }
                }
            }
            for ops in phases {
                let unmet = ops.iter().any(|o| {
                    o.get("kind").and_then(|v| v.as_str()) == Some("require_characters_in_sequence")
                        && chars < o.get("n").and_then(as_strict_i32).unwrap_or(0)
                });
                if unmet {
                    continue;
                }
                let options: Vec<String> = ops
                    .iter()
                    .filter(|o| o.get("kind").and_then(|v| v.as_str()) == Some("choice"))
                    .filter_map(|o| o.get("choices").and_then(|v| v.as_array()))
                    .flatten()
                    .filter_map(|m| m.as_str().map(|s| s.to_string()))
                    .collect();
                let mut choice = Map::new();
                if !options.is_empty() {
                    let m = self.choose_mode(
                        g,
                        c,
                        &options,
                        Some(projected_hand),
                        Some(seq),
                        Some(i),
                        Some(shields),
                        Some(projected_deck),
                        Some(&recovered),
                        Some(enemy_hand),
                    );
                    choice.insert("mode".to_string(), json!(m));
                }
                for op in g.expanded(&ops, &choice).unwrap_or_default() {
                    score += self.op_value(
                        g,
                        c,
                        &op,
                        Some(projected_hand),
                        Some(seq),
                        Some(i),
                        Some(shields),
                        Some(projected_deck),
                        Some(&recovered),
                        Some(enemy_hand),
                    );
                    let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    match k {
                        "break" => {
                            let n = op.get("n").and_then(as_strict_i32).unwrap_or(0) as usize;
                            shields = shields.saturating_sub(n);
                        }
                        "break_finisher" => {
                            let threshold =
                                op.get("threshold").and_then(as_strict_i32).unwrap_or(0) as usize;
                            let amt = if shields <= threshold {
                                op.get("finish").and_then(as_strict_i32).unwrap_or(0) as usize
                            } else {
                                op.get("n").and_then(as_strict_i32).unwrap_or(0) as usize
                            };
                            shields = shields.saturating_sub(amt);
                        }
                        "draw" => {
                            let n = op.get("n").and_then(as_strict_i32).unwrap_or(0) as usize;
                            let gain = n.min(projected_deck);
                            projected_hand += gain;
                            projected_deck -= gain;
                        }
                        "select" => {
                            let gain = usize::from(projected_deck > 0);
                            projected_hand += gain;
                            projected_deck -= gain;
                        }
                        _ if k.starts_with("recover") => {
                            let targets: Vec<Card> = self
                                .recover_targets(g, owner, k)
                                .into_iter()
                                .filter(|t| !recovered.contains(&t.uid))
                                .collect();
                            if let Some(best_t) = targets.iter().max_by(|a, b| {
                                card_value(a)
                                    .partial_cmp(&card_value(b))
                                    .unwrap_or(std::cmp::Ordering::Equal)
                                    .then_with(|| b.uid.cmp(&a.uid))
                            }) {
                                recovered.insert(best_t.uid);
                                projected_hand += 1;
                            }
                        }
                        "discard_enemy" => {
                            let n = op.get("n").and_then(as_strict_i32).unwrap_or(1) as usize;
                            enemy_hand = enemy_hand.saturating_sub(n);
                        }
                        _ => {}
                    }
                }
            }
            score -= (c.cost() as f64) * 0.12;
        }
        score
    }

    pub fn heuristic_sequence(&self, g: &Game, owner: usize) -> Vec<Card> {
        let hand: Vec<Card> = g.p[owner]
            .hand
            .iter()
            .filter(|c| c.card_type() != "Counter")
            .cloned()
            .collect();
        let budget = g.p[owner].actions;
        let (_, _, mut reserve) = policy_weights(&self.names[owner]);
        if !g.p[owner]
            .hand
            .iter()
            .any(|c| c.card_type() == "Counter" || c.response_play())
        {
            reserve = 0;
        }
        let mut beam: Vec<(Vec<Card>, i32)> = vec![(Vec::new(), 0)];
        let mut best: Vec<Card> = Vec::new();
        let mut best_score = 0.0_f64;
        for _ in 0..6 {
            let mut expanded: Vec<(Vec<Card>, i32, f64)> = Vec::new();
            for (seq, cost) in &beam {
                for c in &hand {
                    if seq.iter().any(|x| x.uid == c.uid) || cost + c.cost() > budget {
                        continue;
                    }
                    let mut new_seq = seq.clone();
                    new_seq.push(c.clone());
                    let spent = cost + c.cost();
                    let mut score = self.heuristic_score(g, &new_seq, owner);
                    let short = (reserve - (budget - spent)).max(0);
                    score -= (short as f64) * 0.8;
                    if score > best_score + 1e-9 {
                        best_score = score;
                        best = new_seq.clone();
                    }
                    expanded.push((new_seq, spent, score));
                }
            }
            if expanded.is_empty() {
                break;
            }
            expanded.sort_by(|a, b| {
                b.2.partial_cmp(&a.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        let ua: Vec<i32> = a.0.iter().map(|c| c.uid).collect();
                        let ub: Vec<i32> = b.0.iter().map(|c| c.uid).collect();
                        ua.cmp(&ub)
                    })
            });
            beam = expanded
                .into_iter()
                .take(18)
                .map(|(s, c, _)| (s, c))
                .collect();
        }
        best
    }
}

impl DecisionProviders for AuditPolicy {
    fn has_choice(&self) -> bool {
        true
    }
    fn has_response(&self) -> bool {
        true
    }
    fn has_pick(&self) -> bool {
        self.audit_mode
    }
    fn has_discard(&self) -> bool {
        self.audit_mode
    }

    fn choose(
        &mut self,
        g: &Game,
        c: &Card,
        kind: &str,
        ops: &[Value],
        pending: Option<&Effect>,
    ) -> Result<Map<String, Value>, Illegal> {
        let mut choice = self.base_choose(g, c, kind, ops, pending);
        if self.audit_mode {
            let expanded = g.expanded(ops, &choice).unwrap_or_default();
            for op in &expanded {
                let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                if k.starts_with("recover") {
                    let targets = self.recover_targets(g, c.owner, k);
                    if !targets.is_empty() {
                        let mut best_uid = targets[0].uid;
                        let mut best_rank = self.recovery_rank(g, &targets[0]);
                        for t in &targets[1..] {
                            let r = self.recovery_rank(g, t);
                            if r > best_rank {
                                best_rank = r;
                                best_uid = t.uid;
                            }
                        }
                        choice.insert("target".to_string(), json!(best_uid));
                    }
                }
            }
        }
        Ok(choice)
    }

    fn respond(
        &mut self,
        g: &Game,
        pending: &Effect,
        is_reply: bool,
    ) -> Result<Option<Vec<Response>>, Illegal> {
        let owner = 1 - pending.source.owner;
        let name = self.names[owner].clone();
        *self
            .metrics
            .entry("response_windows".to_string())
            .or_insert(0) += 1;
        let mut candidates = Vec::new();
        for c in &g.p[owner].hand {
            if c.cost() > g.p[owner].actions {
                continue;
            }
            if c.card_type() != "Counter" && (is_reply || !c.response_play()) {
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
            if !g.counter_eligible(c, pending, &raw) {
                continue;
            }
            let choice = self.choose(g, c, "counter", &raw, Some(pending))?;
            let ops = g.expanded(&raw, &choice)?;
            if !g.counter_eligible(c, pending, &ops) {
                continue;
            }
            let mut e = Effect::new(
                c.clone(),
                "counter",
                ops.clone(),
                choice.clone(),
                Some(Box::new(pending.clone())),
            );
            g.prepare(&mut e);
            let has_cbl = ops
                .iter()
                .any(|o| o.get("kind").and_then(|v| v.as_str()) == Some("counter_bounce_later"));
            if (c.response_play() || has_cbl)
                && e.choice.get("target").map(|v| v.is_null()).unwrap_or(true)
            {
                continue;
            }
            candidates.push(c.clone());
        }
        if candidates.is_empty() {
            return Ok(None);
        }
        *self
            .metrics
            .entry("eligible_response_opportunities".to_string())
            .or_insert(0) += 1;
        let value = self.effect_value(g, pending);
        let mut threshold = match name.as_str() {
            "offense" => 5.0,
            "mixed" => 2.8,
            "control" => 0.5,
            "resource" => 0.5,
            _ => 2.8,
        };
        if is_reply {
            threshold = 1.0;
        }
        if value < threshold {
            return Ok(None);
        }
        let chosen = candidates
            .into_iter()
            .min_by(|a, b| {
                a.cost()
                    .cmp(&b.cost())
                    .then_with(|| {
                        let a_is_c = i32::from(a.card_type() == "Counter");
                        let b_is_c = i32::from(b.card_type() == "Counter");
                        b_is_c.cmp(&a_is_c)
                    })
                    .then_with(|| a.uid.cmp(&b.uid))
            })
            .unwrap();
        *self
            .metrics
            .entry("responses_chosen".to_string())
            .or_insert(0) += 1;
        *self
            .metrics
            .entry("response_actions_spent".to_string())
            .or_insert(0) += chosen.cost() as i64;
        Ok(Some(vec![Response::new(chosen.uid)]))
    }

    fn discard(&mut self, g: &Game, owner: usize, n: usize) -> Result<Vec<i32>, Illegal> {
        let mut remaining = g.p[owner].hand.clone();
        let mut chosen = Vec::new();
        let count = n.min(remaining.len());
        for _ in 0..count {
            let progress: Vec<i32> = remaining
                .iter()
                .filter(|c| self.is_progress(c))
                .map(|c| c.uid)
                .collect();
            let access: Vec<i32> = remaining
                .iter()
                .filter(|c| self.is_access(g, c))
                .map(|c| c.uid)
                .collect();
            let best_idx = remaining
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let prot_a = (progress.len() == 1 && progress.contains(&a.uid))
                        || (access.len() == 1 && access.contains(&a.uid));
                    let prot_b = (progress.len() == 1 && progress.contains(&b.uid))
                        || (access.len() == 1 && access.contains(&b.uid));
                    prot_a
                        .cmp(&prot_b)
                        .then_with(|| {
                            card_value(a)
                                .partial_cmp(&card_value(b))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .then_with(|| a.uid.cmp(&b.uid))
                })
                .map(|(i, _)| i)
                .unwrap();
            let c = remaining.remove(best_idx);
            chosen.push(c.uid);
        }
        Ok(chosen)
    }

    fn pick(&mut self, g: &Game, owner: usize, cards: &[Card]) -> Result<i32, Illegal> {
        let needs_progress = !g.p[owner].hand.iter().any(|c| self.is_progress(c));
        let best = cards
            .iter()
            .max_by(|a, b| {
                let pa = i32::from(needs_progress && self.is_progress(a));
                let pb = i32::from(needs_progress && self.is_progress(b));
                pa.cmp(&pb)
                    .then_with(|| {
                        card_value(a)
                            .partial_cmp(&card_value(b))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| b.uid.cmp(&a.uid))
            })
            .unwrap();
        Ok(best.uid)
    }
}
