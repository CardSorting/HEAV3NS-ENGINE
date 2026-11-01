use crate::rng::PyRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const KINDS: &[&str] = &[
    "draw",
    "break",
    "break_finisher",
    "discard",
    "discard_enemy",
    "select",
    "recover_character",
    "recover_action",
    "recover_any",
    "bounce_friendly",
    "bounce_other",
    "bounce_enemy",
    "bounce_enemy_max2",
    "bounce_enemy1",
    "counter_bounce_later",
    "cancel",
    "require_characters_in_sequence",
    "choice",
];

pub fn is_bounce(k: &str) -> bool {
    k.starts_with("bounce")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Illegal(pub String);

impl std::fmt::Display for Illegal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Illegal {}

impl From<&str> for Illegal {
    fn from(s: &str) -> Self {
        Illegal(s.to_string())
    }
}

impl From<String> for Illegal {
    fn from(s: String) -> Self {
        Illegal(s)
    }
}

impl From<Illegal> for String {
    fn from(e: Illegal) -> Self {
        e.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub uid: i32,
    pub owner: usize,
    pub spec: Value,
    pub cancelled: bool,
    pub combo_used: bool,
}

impl Card {
    pub fn new(uid: i32, owner: usize, spec: Value) -> Self {
        Self {
            uid,
            owner,
            spec,
            cancelled: false,
            combo_used: false,
        }
    }

    pub fn id(&self) -> &str {
        self.spec.get("id").and_then(|v| v.as_str()).unwrap_or("")
    }

    pub fn title(&self) -> &str {
        self.spec
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    pub fn card_type(&self) -> &str {
        self.spec.get("type").and_then(|v| v.as_str()).unwrap_or("")
    }

    pub fn cost(&self) -> i32 {
        self.spec.get("cost").and_then(|v| v.as_i64()).unwrap_or(0) as i32
    }

    pub fn text(&self) -> &str {
        self.spec.get("text").and_then(|v| v.as_str()).unwrap_or("")
    }

    pub fn response_play(&self) -> bool {
        self.spec
            .get("response_play")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub fn shield_trigger(&self) -> Option<&str> {
        self.spec.get("shield_trigger").and_then(|v| v.as_str())
    }

    pub fn combo_n(&self) -> Option<i32> {
        self.spec
            .get("combo")
            .and_then(|v| v.get("n"))
            .and_then(|v| v.as_i64())
            .map(|n| n as i32)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub actions: i32,
    pub hand: Vec<Card>,
    pub deck: Vec<Card>,
    pub field: Vec<Card>,
    pub shields: Vec<Card>,
    pub discard: Vec<Card>,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            actions: 6,
            hand: Vec::new(),
            deck: Vec::new(),
            field: Vec::new(),
            shields: Vec::new(),
            discard: Vec::new(),
        }
    }
}

impl Player {
    pub fn zone(&self, name: &str) -> &Vec<Card> {
        match name {
            "hand" => &self.hand,
            "deck" => &self.deck,
            "field" => &self.field,
            "shields" => &self.shields,
            "discard" => &self.discard,
            _ => panic!("unknown zone {name}"),
        }
    }

    pub fn zone_mut(&mut self, name: &str) -> &mut Vec<Card> {
        match name {
            "hand" => &mut self.hand,
            "deck" => &mut self.deck,
            "field" => &mut self.field,
            "shields" => &mut self.shields,
            "discard" => &mut self.discard,
            _ => panic!("unknown zone {name}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub uid: i32,
    pub reply: Option<Vec<Response>>,
}

impl Response {
    pub fn new(uid: i32) -> Self {
        Self { uid, reply: None }
    }

    pub fn with_reply(uid: i32, reply: Response) -> Self {
        Self {
            uid,
            reply: Some(vec![reply]),
        }
    }

    pub fn with_replies(uid: i32, replies: Vec<Response>) -> Self {
        Self {
            uid,
            reply: Some(replies),
        }
    }

    pub fn to_safe_json(&self) -> Value {
        let reply_val = match &self.reply {
            None => Value::Null,
            Some(list) if list.len() == 1 => list[0].to_safe_json(),
            Some(list) => Value::Array(list.iter().map(|r| r.to_safe_json()).collect()),
        };
        json!({
            "uid": self.uid,
            "reply": reply_val
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effect {
    pub source: Card,
    pub kind: String,
    pub ops: Vec<Value>,
    pub choice: Map<String, Value>,
    pub cancelled: bool,
    pub target_effect: Option<Box<Effect>>,
}

impl Effect {
    pub fn new(
        source: Card,
        kind: impl Into<String>,
        ops: Vec<Value>,
        choice: Map<String, Value>,
        target_effect: Option<Box<Effect>>,
    ) -> Self {
        Self {
            source,
            kind: kind.into(),
            ops,
            choice,
            cancelled: false,
            target_effect,
        }
    }
}

pub fn is_strict_int(v: &Value) -> bool {
    !v.is_boolean() && (v.is_i64() || v.is_u64())
}

pub fn as_strict_i32(v: &Value) -> Option<i32> {
    if v.is_boolean() {
        return None;
    }
    v.as_i64().map(|x| x as i32)
}

pub fn decode_mode(mode: &str) -> Result<Value, Illegal> {
    if let Some(rest) = mode.strip_prefix("draw") {
        let n = rest
            .parse::<i32>()
            .map_err(|_| Illegal("invalid choice mode".into()))?;
        return Ok(json!({"kind": "draw", "n": n}));
    }
    if let Some(rest) = mode.strip_prefix("break") {
        let n = rest
            .parse::<i32>()
            .map_err(|_| Illegal("invalid choice mode".into()))?;
        return Ok(json!({"kind": "break", "n": n}));
    }
    if let Some(suffix) = mode.strip_prefix("discard_enemy") {
        let n = if suffix.is_empty() {
            1
        } else {
            suffix
                .parse::<i32>()
                .map_err(|_| Illegal("invalid choice mode".into()))?
        };
        return Ok(json!({"kind": "discard_enemy", "n": n}));
    }
    Ok(json!({"kind": mode}))
}

pub fn validate_ops(ops: &[Value]) -> Result<(), Illegal> {
    for op in ops {
        let obj = op
            .as_object()
            .ok_or_else(|| Illegal("unsupported operation".into()))?;
        let k = obj
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Illegal("unsupported operation".into()))?;
        if !KINDS.contains(&k) {
            return Err(Illegal("unsupported operation".into()));
        }
        let mut allowed: HashSet<&str> = ["kind", "optional"].into_iter().collect();
        if matches!(
            k,
            "draw"
                | "break"
                | "select"
                | "require_characters_in_sequence"
                | "break_finisher"
                | "discard"
                | "discard_enemy"
        ) {
            allowed.insert("n");
        }
        if k == "choice" {
            allowed.insert("choices");
        }
        if k == "cancel" || k == "counter_bounce_later" {
            allowed.insert("scope");
        }
        if k == "break_finisher" {
            allowed.insert("finish");
            allowed.insert("threshold");
        }
        for key in obj.keys() {
            if !allowed.contains(key.as_str()) {
                return Err(Illegal("unknown operation key".into()));
            }
        }
        if let Some(opt_val) = obj.get("optional") {
            if !opt_val.is_boolean() {
                return Err(Illegal("optional must be boolean".into()));
            }
        }
        if k == "choice" {
            let modes = obj
                .get("choices")
                .and_then(|v| v.as_array())
                .ok_or_else(|| Illegal("invalid choices".into()))?;
            if modes.is_empty() || modes.iter().any(|m| !m.is_string()) {
                return Err(Illegal("invalid choices".into()));
            }
            for m in modes {
                let ms = m.as_str().unwrap();
                let decoded = decode_mode(ms)?;
                if decoded.get("kind").and_then(|v| v.as_str()) == Some("choice") {
                    return Err(Illegal("nested choice mode unsupported".into()));
                }
                validate_ops(&[decoded])?;
            }
        }
        if matches!(
            k,
            "draw" | "break" | "select" | "require_characters_in_sequence" | "break_finisher"
        ) {
            let n = obj
                .get("n")
                .and_then(as_strict_i32)
                .ok_or_else(|| Illegal("invalid operation count".into()))?;
            if n < 0 {
                return Err(Illegal("invalid operation count".into()));
            }
        }
        if k == "discard" || k == "discard_enemy" {
            if let Some(n_val) = obj.get("n") {
                let n =
                    as_strict_i32(n_val).ok_or_else(|| Illegal("invalid discard count".into()))?;
                if n < 0 {
                    return Err(Illegal("invalid discard count".into()));
                }
            }
        }
        if k == "break_finisher" {
            for field in ["finish", "threshold"] {
                let v = obj
                    .get(field)
                    .and_then(as_strict_i32)
                    .ok_or_else(|| Illegal("invalid finisher".into()))?;
                if v < 0 {
                    return Err(Illegal("invalid finisher".into()));
                }
            }
        }
        if k == "counter_bounce_later" && obj.get("scope").and_then(|v| v.as_str()) != Some("combo")
        {
            return Err(Illegal("invalid return Counter scope".into()));
        }
        if k == "cancel" {
            let scope = obj.get("scope").and_then(|v| v.as_str()).unwrap_or("");
            if !matches!(
                scope,
                "effect" | "entry" | "action" | "combo" | "target_character" | "counter"
            ) {
                return Err(Illegal("invalid cancel scope".into()));
            }
        }
    }
    Ok(())
}

pub fn validate_choice(choice: &Map<String, Value>, internal: bool) -> Result<(), Illegal> {
    let allowed: HashSet<&str> = if internal {
        ["mode", "target", "pick", "discard", "_options"]
            .into_iter()
            .collect()
    } else {
        ["mode", "target", "pick", "discard"].into_iter().collect()
    };
    for k in choice.keys() {
        if !allowed.contains(k.as_str()) {
            return Err(Illegal("unknown choice key".into()));
        }
    }
    if let Some(m) = choice.get("mode") {
        if !m.is_string() {
            return Err(Illegal("mode must be a string".into()));
        }
    }
    for key in ["target", "pick"] {
        if let Some(val) = choice.get(key) {
            if !val.is_null() {
                let uid = as_strict_i32(val).ok_or_else(|| Illegal("invalid choice uid".into()))?;
                if uid <= 0 {
                    return Err(Illegal("invalid choice uid".into()));
                }
            }
        }
    }
    if let Some(disc) = choice.get("discard") {
        let arr = disc
            .as_array()
            .ok_or_else(|| Illegal("invalid discard choices".into()))?;
        let mut seen = HashSet::new();
        for item in arr {
            let uid =
                as_strict_i32(item).ok_or_else(|| Illegal("invalid discard choices".into()))?;
            if uid <= 0 || !seen.insert(uid) {
                return Err(Illegal("invalid discard choices".into()));
            }
        }
    }
    Ok(())
}

pub fn validate_responses(list: &[Response]) -> Result<(), Illegal> {
    for item in list {
        if item.uid <= 0 {
            return Err(Illegal("invalid response plan".into()));
        }
        if let Some(reply) = &item.reply {
            validate_responses(reply)?;
        }
    }
    Ok(())
}

pub fn validate_specs(specs: &Value) -> Result<Vec<Value>, Illegal> {
    let arr = specs
        .as_array()
        .ok_or_else(|| Illegal("specs must be a list".into()))?;
    let mut seen = HashSet::new();
    for spec in arr {
        let obj = spec
            .as_object()
            .ok_or_else(|| Illegal("missing/duplicate card id".into()))?;
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Illegal("missing/duplicate card id".into()))?;
        if id.is_empty() || !seen.insert(id.to_string()) {
            return Err(Illegal("missing/duplicate card id".into()));
        }
        let ctype = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if !matches!(ctype, "Action" | "Character" | "Counter") {
            return Err(Illegal("invalid card type".into()));
        }
        let cost = obj
            .get("cost")
            .and_then(as_strict_i32)
            .ok_or_else(|| Illegal("cost must be a positive integer".into()))?;
        if cost <= 0 {
            return Err(Illegal("cost must be a positive integer".into()));
        }
        if let Some(rp) = obj.get("response_play") {
            if !rp.is_boolean() {
                return Err(Illegal("invalid response flag".into()));
            }
        }
        if let Some(st) = obj.get("shield_trigger") {
            if !st.is_null() && st.as_str() != Some("Draw 1 card.") {
                return Err(Illegal("unsupported Shield trigger".into()));
            }
        }
        if let Some(ops_val) = obj.get("ops") {
            let ops_arr = ops_val
                .as_array()
                .ok_or_else(|| Illegal("operations must be a list".into()))?;
            validate_ops(ops_arr)?;
        }
        if let Some(eops_val) = obj.get("entry_ops") {
            let eops_arr = eops_val
                .as_array()
                .ok_or_else(|| Illegal("operations must be a list".into()))?;
            validate_ops(eops_arr)?;
        }
        if let Some(co) = obj.get("combo") {
            if !co.is_null() {
                let co_obj = co
                    .as_object()
                    .ok_or_else(|| Illegal("invalid Combo requirement".into()))?;
                let n = co_obj
                    .get("n")
                    .and_then(as_strict_i32)
                    .ok_or_else(|| Illegal("invalid Combo requirement".into()))?;
                if n <= 0 {
                    return Err(Illegal("invalid Combo requirement".into()));
                }
                let co_ops = co_obj
                    .get("ops")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| Illegal("operations must be a list".into()))?;
                validate_ops(co_ops)?;
            }
        }
    }
    Ok(arr.clone())
}

pub fn load_default_specs() -> Vec<Value> {
    let raw = include_str!("../data/cards.json");
    let val: Value = serde_json::from_str(raw).expect("valid embedded data/cards.json");
    validate_specs(&val).expect("valid default card specs")
}

pub fn load_proposed_specs() -> Vec<Value> {
    let raw = include_str!("../data/proposed.json");
    let val: Value = serde_json::from_str(raw).expect("valid embedded data/proposed.json");
    validate_specs(&val).expect("valid proposed card specs")
}

pub fn python_escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c if (c as u32) <= 0x7e => {
                out.push(c);
            }
            c if (c as u32) <= 0xffff => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => {
                let code = (c as u32) - 0x10000;
                let high = 0xd800 + (code >> 10);
                let low = 0xdc00 + (code & 0x3ff);
                out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
            }
        }
    }
    out.push('"');
    out
}

/// Canonical compact JSON serializer with sorted dictionary keys and ASCII escaping (matching `json.dumps(..., sort_keys=True, separators=(',', ':'))`).
pub fn canonical_json_compact(val: &Value) -> String {
    match val {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => python_escape_json_string(s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_json_compact).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let items: Vec<String> = keys
                .into_iter()
                .map(|k| {
                    let ks = python_escape_json_string(k);
                    let vs = canonical_json_compact(&map[k]);
                    format!("{ks}:{vs}")
                })
                .collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

/// Computes `r2_contract.specs_hash(specs)`: SHA-256 of `sorted(specs, key=lambda x: x['id'])` with `sort_keys=True, separators=(',',':')`.
pub fn specs_hash(specs: &[Value]) -> String {
    let mut sorted = specs.to_vec();
    sorted.sort_by(|a, b| {
        let ida = a.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let idb = b.get("id").and_then(|v| v.as_str()).unwrap_or("");
        ida.cmp(idb)
    });
    let dump = canonical_json_compact(&Value::Array(sorted));
    let mut hasher = Sha256::new();
    hasher.update(dump.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Trait for decision providers (`choice`, `response`, `pick`, `discard`) attached to `Game` / `SimGame`.
pub trait DecisionProviders {
    fn has_choice(&self) -> bool {
        false
    }
    fn has_response(&self) -> bool {
        false
    }
    fn has_pick(&self) -> bool {
        false
    }
    fn has_discard(&self) -> bool {
        false
    }

    fn choose(
        &mut self,
        _g: &Game,
        _c: &Card,
        _kind: &str,
        _ops: &[Value],
        _pending: Option<&Effect>,
    ) -> Result<Map<String, Value>, Illegal> {
        Ok(Map::new())
    }

    fn respond(
        &mut self,
        _g: &Game,
        _pending: &Effect,
        _is_reply: bool,
    ) -> Result<Option<Vec<Response>>, Illegal> {
        Ok(None)
    }

    fn pick(&mut self, _g: &Game, _owner: usize, _cards: &[Card]) -> Result<i32, Illegal> {
        Ok(0)
    }

    fn discard(&mut self, _g: &Game, _owner: usize, _n: usize) -> Result<Vec<i32>, Illegal> {
        Ok(Vec::new())
    }
}

pub struct NoProviders;
impl DecisionProviders for NoProviders {}

/// Complete deterministic Wonderland FC04.1 rules runtime (`Game` + `SimGame` capabilities).
#[derive(Clone)]
pub struct Game {
    pub opening_protection: bool,
    pub sim_mode: bool,
    pub rng: PyRng,
    pub trace_transitions: bool,
    pub transitions: Vec<Value>,
    pub turn_number: i32,
    pub specs_list: Vec<Value>,
    pub specs: BTreeMap<String, Value>,
    pub p: [Player; 2],
    pub cards: BTreeMap<i32, Card>,
    pub sequence: Vec<Card>,
    pub phase: String,
    pub active: Option<usize>,
    pub winner: Option<usize>,
    pub log: Vec<Vec<Value>>,
    pub responses: HashMap<(i32, String), Vec<Response>>,
    pub choices: HashMap<(i32, String), Map<String, Value>>,
    pub attempt_decisions: Vec<Value>,
    pub provider_outputs: Vec<Value>,
    pub command_providers: Vec<String>,
    pub validation_passed: bool,
}

#[derive(Clone)]
struct GameSnapshot {
    opening_protection: bool,
    rng: PyRng,
    turn_number: i32,
    specs_list: Vec<Value>,
    specs: BTreeMap<String, Value>,
    p: [Player; 2],
    cards: BTreeMap<i32, Card>,
    sequence: Vec<Card>,
    phase: String,
    active: Option<usize>,
    winner: Option<usize>,
    log: Vec<Vec<Value>>,
    responses: HashMap<(i32, String), Vec<Response>>,
    choices: HashMap<(i32, String), Map<String, Value>>,
}

impl Game {
    pub fn new(specs: Option<Vec<Value>>, opening_protection: bool) -> Result<Self, Illegal> {
        let validated = validate_specs(&Value::Array(specs.unwrap_or_else(load_default_specs)))?;
        let mut specs_map = BTreeMap::new();
        for s in &validated {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap().to_string();
            specs_map.insert(id, s.clone());
        }
        Ok(Self {
            opening_protection,
            sim_mode: false,
            rng: PyRng::new(0),
            trace_transitions: false,
            transitions: Vec::new(),
            turn_number: 0,
            specs_list: validated,
            specs: specs_map,
            p: [Player::default(), Player::default()],
            cards: BTreeMap::new(),
            sequence: Vec::new(),
            phase: "ended".to_string(),
            active: None,
            winner: None,
            log: Vec::new(),
            responses: HashMap::new(),
            choices: HashMap::new(),
            attempt_decisions: Vec::new(),
            provider_outputs: Vec::new(),
            command_providers: Vec::new(),
            validation_passed: false,
        })
    }

    pub fn new_sim(specs: Option<Vec<Value>>, opening_protection: bool) -> Result<Self, Illegal> {
        let mut g = Self::new(specs, opening_protection)?;
        g.sim_mode = true;
        Ok(g)
    }

    fn take_snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            opening_protection: self.opening_protection,
            rng: self.rng.clone(),
            turn_number: self.turn_number,
            specs_list: self.specs_list.clone(),
            specs: self.specs.clone(),
            p: self.p.clone(),
            cards: self.cards.clone(),
            sequence: self.sequence.clone(),
            phase: self.phase.clone(),
            active: self.active,
            winner: self.winner,
            log: self.log.clone(),
            responses: self.responses.clone(),
            choices: self.choices.clone(),
        }
    }

    fn restore_snapshot(&mut self, snap: GameSnapshot) {
        self.opening_protection = snap.opening_protection;
        self.rng = snap.rng;
        self.turn_number = snap.turn_number;
        self.specs_list = snap.specs_list;
        self.specs = snap.specs;
        self.p = snap.p;
        self.cards = snap.cards;
        self.sequence = snap.sequence;
        self.phase = snap.phase;
        self.active = snap.active;
        self.winner = snap.winner;
        self.log = snap.log;
        self.responses = snap.responses;
        self.choices = snap.choices;
    }

    fn sync_card(&mut self, updated: &Card) {
        if let Some(c) = self.cards.get_mut(&updated.uid) {
            *c = updated.clone();
        }
        for pl in &mut self.p {
            for z in [
                &mut pl.hand,
                &mut pl.deck,
                &mut pl.field,
                &mut pl.shields,
                &mut pl.discard,
            ] {
                for item in z.iter_mut() {
                    if item.uid == updated.uid {
                        *item = updated.clone();
                    }
                }
            }
        }
        for item in &mut self.sequence {
            if item.uid == updated.uid {
                *item = updated.clone();
            }
        }
    }

    pub fn get_card(&self, uid: i32) -> Result<Card, Illegal> {
        self.cards
            .get(&uid)
            .cloned()
            .ok_or_else(|| Illegal("unknown card uid".into()))
    }

    pub fn present(&self, uid: i32) -> bool {
        if let Some(c) = self.cards.get(&uid) {
            self.p[c.owner].field.iter().any(|x| x.uid == uid)
        } else {
            false
        }
    }

    pub fn supporters(&self, c: &Card) -> Vec<Card> {
        if let Some(pos) = self.sequence.iter().position(|x| x.uid == c.uid) {
            self.sequence[pos + 1..]
                .iter()
                .filter(|s| s.owner == c.owner && s.card_type() != "Counter" && self.present(s.uid))
                .filter_map(|s| self.cards.get(&s.uid).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn combo_eligible(&self, c: &Card) -> bool {
        let live = match self.cards.get(&c.uid) {
            Some(x) => x,
            None => return false,
        };
        if let Some(n) = live.combo_n() {
            self.present(live.uid) && !live.combo_used && (self.supporters(live).len() as i32) >= n
        } else {
            false
        }
    }

    pub fn entry_ops(&self, c: &Card) -> Result<Vec<Value>, Illegal> {
        if let Some(eops) = c.spec.get("entry_ops") {
            if eops.is_null() {
                return Ok(Vec::new());
            }
            return Ok(eops.as_array().cloned().unwrap_or_default());
        }
        if let Some(entry_prose) = c.spec.get("entry") {
            if !entry_prose.is_null() && entry_prose.as_str().map(|s| !s.is_empty()).unwrap_or(true)
            {
                return Err(Illegal(
                    "entry requires explicit entry_ops; legacy prose is not executable".into(),
                ));
            }
        }
        Ok(Vec::new())
    }

    pub fn expanded(
        &self,
        ops: &[Value],
        choice: &Map<String, Value>,
    ) -> Result<Vec<Value>, Illegal> {
        validate_ops(ops)?;
        validate_choice(choice, false)?;
        let mut result = Vec::new();
        for op in ops {
            let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if k == "choice" {
                let choices = op
                    .get("choices")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| Illegal("invalid choices".into()))?;
                let default_mode = choices[0].as_str().unwrap_or("");
                let mode = choice
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or(default_mode);
                if !choices.iter().any(|m| m.as_str() == Some(mode)) {
                    return Err(Illegal("invalid choice mode".into()));
                }
                result.push(decode_mode(mode)?);
            } else {
                result.push(op.clone());
            }
        }
        Ok(result)
    }

    pub fn transition_state(&self) -> Value {
        let mut zones = Vec::new();
        for p in &self.p {
            let mut zmap = Map::new();
            for zname in ["hand", "deck", "field", "shields", "discard"] {
                let ids: Vec<Value> = p.zone(zname).iter().map(|c| Value::from(c.uid)).collect();
                zmap.insert(zname.to_string(), Value::Array(ids));
            }
            zones.push(Value::Object(zmap));
        }
        let mut flags = Map::new();
        for (uid, c) in &self.cards {
            flags.insert(uid.to_string(), json!([c.cancelled, c.combo_used]));
        }
        json!({
            "turn": self.turn_number,
            "player": self.active,
            "phase": self.phase,
            "actions": [self.p[0].actions, self.p[1].actions],
            "rng_sha256": self.rng.rng_sha256(),
            "winner": self.winner,
            "sequence": self.sequence.iter().map(|c| c.uid).collect::<Vec<i32>>(),
            "zones": zones,
            "flags": Value::Object(flags)
        })
    }

    pub fn transition_eligible(&self) -> Value {
        let uids: Vec<i32> = if let Some(act) = self.active {
            if self.phase == "placement" && self.winner.is_none() {
                self.p[act]
                    .hand
                    .iter()
                    .filter(|c| c.card_type() != "Counter" && c.cost() <= self.p[act].actions)
                    .map(|c| c.uid)
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        json!({
            "scope": "Affordable own non-Counter placements only; not the full legal action/target/response space",
            "uids": uids
        })
    }

    fn execute_command<T, F>(
        &mut self,
        cmd_name: &str,
        args_json: Value,
        kwargs_json: Value,
        provider_roles: Vec<String>,
        f: F,
    ) -> Result<T, Illegal>
    where
        F: FnOnce(&mut Self) -> Result<T, Illegal>,
    {
        let snapshot = self.take_snapshot();
        let traced = self.trace_transitions;
        let before = if traced {
            Some(self.transition_state())
        } else {
            None
        };
        let options = if traced {
            Some(self.transition_eligible())
        } else {
            None
        };
        let offset = self.log.len();
        self.attempt_decisions.clear();
        self.provider_outputs.clear();
        self.command_providers = provider_roles;
        self.validation_passed = false;

        let res = f(self);
        let final_res = match res {
            Ok(val) => match self.invariants() {
                Ok(()) => Ok(val),
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        };

        let status;
        let err_obj;
        let events: Vec<Vec<Value>>;
        let decisions: Vec<Value>;

        match &final_res {
            Ok(_) => {
                status = "committed";
                err_obj = Value::Null;
                events = self.log[offset..].to_vec();
                decisions = self.attempt_decisions.clone();
            }
            Err(e) => {
                status = "rejected";
                err_obj = json!({
                    "type": "Illegal",
                    "message": e.0
                });
                events = self.log[offset..].to_vec();
                decisions = self.attempt_decisions.clone();
                self.restore_snapshot(snapshot);
            }
        }

        let validated = self.validation_passed;
        if traced {
            let before_val = before.unwrap();
            let options_val = options.unwrap();
            let after_val = self.transition_state();
            let mut changes = Vec::new();
            for player in 0..2 {
                for zone in ["hand", "deck", "field", "shields", "discard"] {
                    let b_ids = &before_val["zones"][player][zone];
                    let a_ids = &after_val["zones"][player][zone];
                    if b_ids != a_ids {
                        changes.push(json!({
                            "player": player,
                            "zone": zone,
                            "before": b_ids,
                            "after": a_ids
                        }));
                    }
                }
            }
            let player_commands = ["start", "begin_sequence", "place", "close", "end_turn"];
            let cmd_class = if player_commands.contains(&cmd_name) {
                "player"
            } else {
                "fixture/internal-helper"
            };
            let turn_val = if status == "committed" {
                &after_val["turn"]
            } else {
                &before_val["turn"]
            };
            let player_val = if status == "committed" {
                &after_val["player"]
            } else {
                &before_val["player"]
            };
            let validation_str = if validated {
                "accepted"
            } else if status == "rejected" {
                "rejected"
            } else {
                status
            };
            let b_a0 = before_val["actions"][0].as_i64().unwrap_or(0);
            let b_a1 = before_val["actions"][1].as_i64().unwrap_or(0);
            let a_a0 = after_val["actions"][0].as_i64().unwrap_or(0);
            let a_a1 = after_val["actions"][1].as_i64().unwrap_or(0);

            let counter_events: Vec<Vec<Value>> = events
                .iter()
                .filter(|ev| {
                    matches!(
                        ev.first().and_then(|v| v.as_str()),
                        Some("window") | Some("counter") | Some("cancel")
                    )
                })
                .cloned()
                .collect();
            let shield_events: Vec<Vec<Value>> = events
                .iter()
                .filter(|ev| {
                    matches!(
                        ev.first().and_then(|v| v.as_str()),
                        Some("reveal")
                            | Some("shield_discard")
                            | Some("opening_floor")
                            | Some("win")
                    )
                })
                .cloned()
                .collect();

            let rec = json!({
                "schema": "wonderland-transition-1",
                "visibility": "audit-full-state; never provide to gameplay policy",
                "index": self.transitions.len() + 1,
                "rolled_back": status != "committed",
                "events_committed": status == "committed",
                "command": cmd_name,
                "command_class": cmd_class,
                "turn": turn_val,
                "player": player_val,
                "status": status,
                "validation": validation_str,
                "selected": {
                    "args": args_json,
                    "kwargs": kwargs_json
                },
                "before": before_val,
                "after": after_val,
                "eligible": options_val,
                "resources": {
                    "actions_delta": [a_a0 - b_a0, a_a1 - b_a1]
                },
                "provider_roles": self.command_providers,
                "provider_outputs": self.provider_outputs,
                "decisions": decisions,
                "events": events,
                "state_changes": changes,
                "counter_events": counter_events,
                "shield_events": shield_events,
                "winner": self.winner,
                "error": err_obj
            });
            self.transitions.push(rec);
        }
        self.attempt_decisions.clear();
        final_res
    }

    pub fn add(&mut self, owner: usize, card_id: &str, zone: &str) -> Result<i32, Illegal> {
        let args = json!([owner, card_id, zone]);
        self.execute_command("add", args, json!({}), Vec::new(), |g| {
            if g.phase != "ended" {
                return Err(Illegal("setup only between turns".into()));
            }
            if owner > 1 || !matches!(zone, "hand" | "deck" | "field" | "shields" | "discard") {
                return Err(Illegal("invalid owner/zone".into()));
            }
            let spec = g
                .specs
                .get(card_id)
                .cloned()
                .ok_or_else(|| Illegal("unknown card id".into()))?;
            g.validation_passed = true;
            let next_uid = g.cards.keys().max().copied().unwrap_or(0) + 1;
            let c = Card::new(next_uid, owner, spec);
            g.cards.insert(next_uid, c.clone());
            g.p[owner].zone_mut(zone).push(c);
            Ok(next_uid)
        })
    }

    pub fn start(&mut self, owner: usize) -> Result<(), Illegal> {
        self.start_with_providers(owner, &NoProviders)
    }

    pub fn start_with_providers<P: DecisionProviders>(
        &mut self,
        owner: usize,
        providers: &P,
    ) -> Result<(), Illegal> {
        let roles = Self::provider_roles(providers);
        self.execute_command("start", json!([owner]), json!({}), roles, |g| {
            if g.phase != "ended" || g.winner.is_some() || owner > 1 {
                return Err(Illegal("start timing".into()));
            }
            if g.active == Some(owner) {
                return Err(Illegal("alternate turns".into()));
            }
            g.validation_passed = true;
            g.active = Some(owner);
            g.turn_number += 1;
            g.p[owner].actions = 6;
            g.sequence.clear();
            g.phase = "placement".to_string();
            g.log.push(vec![json!("start"), json!(owner)]);
            if g.turn_number > 1 {
                g.draw_internal(owner, 1)?;
            }
            Ok(())
        })
    }

    pub fn begin_sequence(&mut self) -> Result<(), Illegal> {
        self.begin_sequence_with_providers(&NoProviders)
    }

    pub fn begin_sequence_with_providers<P: DecisionProviders>(
        &mut self,
        providers: &P,
    ) -> Result<(), Illegal> {
        let roles = Self::provider_roles(providers);
        self.execute_command("begin_sequence", json!([]), json!({}), roles, |g| {
            if g.phase != "resolved" || g.winner.is_some() {
                return Err(Illegal("finish prior sequence first".into()));
            }
            g.validation_passed = true;
            g.sequence.clear();
            g.phase = "placement".to_string();
            Ok(())
        })
    }

    fn pay(&mut self, c: &Card) -> Result<(), Illegal> {
        let cost = c.cost();
        if cost <= 0 || self.p[c.owner].actions < cost {
            return Err(Illegal("insufficient Actions or invalid cost".into()));
        }
        self.validation_passed = true;
        self.p[c.owner].actions -= cost;
        self.log.push(vec![
            json!("pay"),
            json!(c.owner),
            json!(c.uid),
            json!(cost),
        ]);
        Ok(())
    }

    pub fn place(&mut self, uid: i32) -> Result<(), Illegal> {
        self.place_with_providers(uid, &NoProviders)
    }

    pub fn place_with_providers<P: DecisionProviders>(
        &mut self,
        uid: i32,
        providers: &P,
    ) -> Result<(), Illegal> {
        let roles = Self::provider_roles(providers);
        self.execute_command("place", json!([uid]), json!({}), roles, |g| {
            if g.phase != "placement" || g.winner.is_some() {
                return Err(Illegal("sequence closed".into()));
            }
            let mut c = g.get_card(uid)?;
            if Some(c.owner) != g.active
                || !g.p[c.owner].hand.iter().any(|x| x.uid == uid)
                || c.card_type() == "Counter"
            {
                return Err(Illegal("place own non-Counter from hand".into()));
            }
            g.pay(&c)?;
            g.p[c.owner].hand.retain(|x| x.uid != uid);
            c.cancelled = false;
            c.combo_used = false;
            g.p[c.owner].field.push(c.clone());
            g.sequence.push(c.clone());
            g.sync_card(&c);
            g.log.push(vec![json!("place"), json!(uid)]);
            Ok(())
        })
    }

    pub fn provider_roles<P: DecisionProviders>(providers: &P) -> Vec<String> {
        let mut roles = Vec::new();
        if providers.has_choice() {
            roles.push("choice".to_string());
        }
        if providers.has_response() {
            roles.push("response".to_string());
        }
        if providers.has_pick() {
            roles.push("pick".to_string());
        }
        if providers.has_discard() {
            roles.push("discard".to_string());
        }
        roles
    }

    pub fn close(&mut self) -> Result<(), Illegal> {
        let mut np = NoProviders;
        self.close_with_options(None, None, &mut np)
    }

    pub fn close_with_providers<P: DecisionProviders>(
        &mut self,
        providers: &mut P,
    ) -> Result<(), Illegal> {
        self.close_with_options(None, None, providers)
    }

    pub fn close_with_options<P: DecisionProviders>(
        &mut self,
        responses: Option<HashMap<(i32, String), Vec<Response>>>,
        choices: Option<HashMap<(i32, String), Map<String, Value>>>,
        providers: &mut P,
    ) -> Result<(), Illegal> {
        let roles = Self::provider_roles(providers);
        self.execute_command("close", json!([]), json!({}), roles, |g| {
            if g.phase != "placement" || g.winner.is_some() {
                return Err(Illegal("close timing".into()));
            }
            if let Some(ref ch_map) = choices {
                for ((uid, kind), ch) in ch_map {
                    if !g.cards.contains_key(uid)
                        || !matches!(kind.as_str(), "entry" | "action" | "combo" | "counter")
                    {
                        return Err(Illegal("invalid effect plan key".into()));
                    }
                    validate_choice(ch, false)?;
                }
            }
            if let Some(ref resp_map) = responses {
                for ((uid, kind), rlist) in resp_map {
                    if !g.cards.contains_key(uid)
                        || !matches!(kind.as_str(), "entry" | "action" | "combo" | "counter")
                    {
                        return Err(Illegal("invalid effect plan key".into()));
                    }
                    validate_responses(rlist)?;
                }
            }
            g.validation_passed = true;
            g.phase = "resolving".to_string();
            g.responses = responses.unwrap_or_default();
            g.choices = choices.unwrap_or_default();
            let seq_uids: Vec<Value> = g.sequence.iter().map(|c| json!(c.uid)).collect();
            g.log.push(vec![json!("close"), Value::Array(seq_uids)]);

            let seq_cards: Vec<Card> = g.sequence.clone();
            for seq_c in &seq_cards {
                if g.winner.is_some() {
                    break;
                }
                if !g.present(seq_c.uid) {
                    continue;
                }
                let live_c = g.get_card(seq_c.uid)?;
                let kind = if live_c.card_type() == "Action" {
                    "action"
                } else {
                    "entry"
                };
                let ops = if kind == "action" {
                    live_c
                        .spec
                        .get("ops")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default()
                } else {
                    g.entry_ops(&live_c)?
                };
                if !ops.is_empty() {
                    g.window(live_c.uid, kind, &ops, providers)?;
                }
                let after_entry_c = g.get_card(seq_c.uid)?;
                if g.winner.is_none() && g.combo_eligible(&after_entry_c) {
                    let mut marked = after_entry_c.clone();
                    marked.combo_used = true;
                    g.sync_card(&marked);
                    let combo_ops = marked
                        .spec
                        .get("combo")
                        .and_then(|v| v.get("ops"))
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    g.window(marked.uid, "combo", &combo_ops, providers)?;
                }
            }

            // Ordinary zone cleanup runs even after victory.
            let cleanup_seq = g.sequence.clone();
            for seq_c in cleanup_seq {
                if g.present(seq_c.uid) {
                    let live_c = g.get_card(seq_c.uid)?;
                    g.p[live_c.owner].field.retain(|x| x.uid != live_c.uid);
                    g.p[live_c.owner].discard.push(live_c.clone());
                    g.log
                        .push(vec![json!("sequence_discard"), json!(live_c.uid)]);
                }
            }
            g.phase = "resolved".to_string();
            g.invariants()?;
            Ok(())
        })
    }

    fn window<P: DecisionProviders>(
        &mut self,
        c_uid: i32,
        kind: &str,
        ops: &[Value],
        providers: &mut P,
    ) -> Result<(), Illegal> {
        let c = self.get_card(c_uid)?;
        let choice = if providers.has_choice() {
            let ch = providers.choose(self, &c, kind, ops, None)?;
            if self.trace_transitions {
                self.provider_outputs.push(json!({
                    "role": "choice",
                    "output": ch
                }));
            }
            ch
        } else {
            self.choices
                .get(&(c.uid, kind.to_string()))
                .cloned()
                .unwrap_or_default()
        };
        let expanded_ops = self.expanded(ops, &choice)?;
        let mut e = Effect::new(c.clone(), kind, expanded_ops, choice, None);
        self.prepare(&mut e);
        self.trace_decision(&e);
        if !self.requirements_met(&e) {
            return Ok(());
        }
        let options: Vec<String> = ops
            .iter()
            .filter(|o| o.get("kind").and_then(|v| v.as_str()) == Some("choice"))
            .filter_map(|o| o.get("choices").and_then(|v| v.as_array()))
            .flatten()
            .filter_map(|m| m.as_str().map(|s| s.to_string()))
            .collect();
        if !options.is_empty() {
            e.choice.insert(
                "_options".to_string(),
                Value::Array(options.iter().map(|s| json!(s)).collect()),
            );
            let viable = self.viable_modes(&c, &options, e.target_effect.as_deref());
            let chosen_mode = e
                .choice
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or(&options[0])
                .to_string();
            self.log.push(vec![
                json!("mode_declared"),
                json!(c.uid),
                json!(c.id()),
                json!(c.owner),
                json!(kind),
                json!(chosen_mode),
                json!(viable),
            ]);
        }
        self.log
            .push(vec![json!("window"), json!(c.uid), json!(kind)]);
        let scripted = if providers.has_response() {
            None
        } else {
            self.responses.get(&(c.uid, kind.to_string())).cloned()
        };
        self.run_responses(&mut e, false, scripted, providers)?;
        if self.winner.is_some() || e.cancelled || !self.present(c.uid) {
            return Ok(());
        }
        if kind == "combo" {
            let live_c = self.get_card(c.uid)?;
            let req_n = live_c.combo_n().unwrap_or(0);
            if (self.supporters(&live_c).len() as i32) < req_n {
                self.log
                    .push(vec![json!("combo_lost_support"), json!(c.uid)]);
                return Ok(());
            }
        }
        self.resolve(&mut e, providers)
    }

    fn pending_alive(&self, e: &Effect) -> bool {
        if self.winner.is_some() || e.cancelled || !self.present(e.source.uid) {
            return false;
        }
        if e.kind == "combo" {
            let in_seq = self.sequence.iter().any(|x| x.uid == e.source.uid);
            let req_n = e.source.combo_n().unwrap_or(0);
            if !in_seq || (self.supporters(&e.source).len() as i32) < req_n {
                return false;
            }
        }
        true
    }

    fn run_responses<P: DecisionProviders>(
        &mut self,
        e: &mut Effect,
        is_reply: bool,
        scripted: Option<Vec<Response>>,
        providers: &mut P,
    ) -> Result<(), Illegal> {
        if let Some(ref s) = scripted {
            validate_responses(s)?;
        }
        let mut queue: Vec<Response> = scripted.unwrap_or_default();
        while self.pending_alive(e) {
            let response = if !queue.is_empty() {
                queue.remove(0)
            } else if providers.has_response() {
                let offered = providers.respond(self, e, is_reply)?;
                if let Some(ref list) = offered {
                    validate_responses(list)?;
                }
                if self.trace_transitions {
                    let safe_out = match &offered {
                        None => Value::Null,
                        Some(list) if list.is_empty() => json!([]),
                        Some(list) if list.len() == 1 => list[0].to_safe_json(),
                        Some(list) => Value::Array(list.iter().map(|r| r.to_safe_json()).collect()),
                    };
                    self.provider_outputs.push(json!({
                        "role": "response",
                        "output": safe_out
                    }));
                }
                match offered {
                    None => break,
                    Some(list) if list.is_empty() => break,
                    Some(mut list) => {
                        let first = list.remove(0);
                        queue = list;
                        first
                    }
                }
            } else {
                break;
            };
            self.counter(e, response, is_reply, providers)?;
        }
        Ok(())
    }

    pub fn counter_eligible(&self, c: &Card, pending: &Effect, ops: &[Value]) -> bool {
        if c.owner == pending.source.owner {
            return false;
        }
        for op in ops {
            let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if k == "cancel" {
                let scope = op.get("scope").and_then(|v| v.as_str()).unwrap_or("");
                if scope == "effect"
                    || scope == pending.kind
                    || (scope == "action" && pending.kind == "counter")
                {
                    return true;
                }
                if scope == "target_character" {
                    if let Some(t_uid) = pending.choice.get("target").and_then(as_strict_i32) {
                        if let Some(tc) = self.cards.get(&t_uid) {
                            let has_bounce = pending.ops.iter().any(|o| {
                                let ok = o.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                                is_bounce(ok) || ok == "counter_bounce_later"
                            });
                            if tc.owner == c.owner && tc.card_type() == "Character" && has_bounce {
                                return true;
                            }
                        }
                    }
                }
            } else if (k == "counter_bounce_later" && pending.kind == "combo")
                || (is_bounce(k) && pending.kind != "counter")
            {
                return true;
            }
        }
        false
    }

    fn counter<P: DecisionProviders>(
        &mut self,
        pending: &mut Effect,
        response: Response,
        is_reply: bool,
        providers: &mut P,
    ) -> Result<(), Illegal> {
        let mut c = self.get_card(response.uid)?;
        let raw_ops = if c.card_type() == "Character" {
            self.entry_ops(&c)?
        } else {
            c.spec
                .get("ops")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
        };
        let choice = if providers.has_choice() {
            let ch = providers.choose(self, &c, "counter", &raw_ops, Some(pending))?;
            if self.trace_transitions {
                self.provider_outputs.push(json!({
                    "role": "choice",
                    "output": ch
                }));
            }
            ch
        } else {
            self.choices
                .get(&(c.uid, "counter".to_string()))
                .cloned()
                .unwrap_or_default()
        };
        let ops = self.expanded(&raw_ops, &choice)?;
        let response_play = c.response_play() && !is_reply;
        let in_hand = self.p[c.owner].hand.iter().any(|x| x.uid == c.uid);
        if (c.card_type() != "Counter" && !response_play)
            || !in_hand
            || !self.counter_eligible(&c, pending, &ops)
        {
            return Err(Illegal("ineligible Counter".into()));
        }
        let kind = match c.card_type() {
            "Counter" => "counter",
            "Character" => "entry",
            _ => "action",
        };
        let mut e = Effect::new(
            c.clone(),
            kind,
            ops.clone(),
            choice,
            Some(Box::new(pending.clone())),
        );
        self.prepare(&mut e);
        self.trace_decision(&e);
        let has_cbl = ops
            .iter()
            .any(|o| o.get("kind").and_then(|v| v.as_str()) == Some("counter_bounce_later"));
        if (response_play || has_cbl)
            && (e.choice.get("mode").and_then(|v| v.as_str()) == Some("skip")
                || e.choice.get("target").map(|v| v.is_null()).unwrap_or(true))
        {
            return Err(Illegal("Response requires eligible target".into()));
        }
        self.pay(&c)?;
        self.p[c.owner].hand.retain(|x| x.uid != c.uid);
        c.cancelled = false;
        self.p[c.owner].field.push(c.clone());
        self.sync_card(&c);
        self.log.push(vec![
            json!("counter"),
            json!(c.uid),
            json!(pending.source.uid),
            json!(pending.kind),
        ]);
        self.run_responses(&mut e, true, response.reply, providers)?;
        if self.pending_alive(&e) {
            self.resolve(&mut e, providers)?;
            if let Some(updated_target) = e.target_effect {
                if updated_target.cancelled {
                    pending.cancelled = true;
                    pending.source.cancelled = true;
                    let mut sc = self.get_card(pending.source.uid)?;
                    sc.cancelled = true;
                    self.sync_card(&sc);
                }
            }
        }
        if e.cancelled {
            c.cancelled = true;
            self.sync_card(&c);
        }
        if self.present(c.uid) {
            let live_c = self.get_card(c.uid)?;
            self.p[live_c.owner].field.retain(|x| x.uid != live_c.uid);
            self.p[live_c.owner].discard.push(live_c);
        }
        Ok(())
    }

    pub fn requirements_met(&self, e: &Effect) -> bool {
        e.ops.iter().all(|op| {
            if op.get("kind").and_then(|v| v.as_str()) != Some("require_characters_in_sequence") {
                return true;
            }
            let req_n = op.get("n").and_then(as_strict_i32).unwrap_or(0);
            let count = self
                .sequence
                .iter()
                .filter(|c| {
                    self.present(c.uid) && c.owner == e.source.owner && c.card_type() == "Character"
                })
                .count() as i32;
            count >= req_n
        })
    }

    pub fn bounce_candidates(&self, e: &Effect, kind: &str) -> Vec<Card> {
        let owner = e.source.owner;
        if kind == "counter_bounce_later" {
            let pending = match &e.target_effect {
                Some(p) => p,
                None => return Vec::new(),
            };
            let in_seq = self.sequence.iter().any(|x| x.uid == pending.source.uid);
            if !in_seq || !self.present(pending.source.uid) {
                return Vec::new();
            }
            return self
                .supporters(&pending.source)
                .into_iter()
                .filter(|c| c.card_type() == "Character")
                .collect();
        }
        let side = if kind.contains("enemy") {
            1 - owner
        } else {
            owner
        };
        let mut cards: Vec<Card> = self.p[side]
            .field
            .iter()
            .filter(|c| c.card_type() == "Character")
            .cloned()
            .collect();
        if kind == "bounce_other" {
            cards.retain(|c| c.uid != e.source.uid);
        }
        let limit = if kind == "bounce_enemy_max2" {
            Some(2)
        } else if kind == "bounce_enemy1" {
            Some(1)
        } else {
            None
        };
        if let Some(lim) = limit {
            cards.retain(|c| c.cost() <= lim);
        }
        cards
    }

    pub fn prepare(&self, e: &mut Effect) {
        let ops_clone = e.ops.clone();
        for op in &ops_clone {
            let optional = op
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if optional && e.choice.get("mode").and_then(|v| v.as_str()) == Some("skip") {
                continue;
            }
            let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if is_bounce(k) || k == "counter_bounce_later" {
                let candidates = self.bounce_candidates(e, k);
                if let Some(t_val) = e.choice.get("target") {
                    let t_uid = as_strict_i32(t_val);
                    if !candidates.iter().any(|c| Some(c.uid) == t_uid) {
                        e.choice.insert("target".to_string(), Value::Null);
                    }
                } else {
                    let default_t = candidates
                        .first()
                        .map(|c| Value::from(c.uid))
                        .unwrap_or(Value::Null);
                    e.choice.insert("target".to_string(), default_t);
                }
            } else if matches!(k, "recover_character" | "recover_action" | "recover_any") {
                let candidates: Vec<Card> = self.p[e.source.owner]
                    .discard
                    .iter()
                    .filter(|c| match k {
                        "recover_character" => c.card_type() == "Character",
                        "recover_action" => c.card_type() == "Action" || c.card_type() == "Counter",
                        _ => true,
                    })
                    .cloned()
                    .collect();
                if let Some(t_val) = e.choice.get("target") {
                    let t_uid = as_strict_i32(t_val);
                    if !candidates.iter().any(|c| Some(c.uid) == t_uid) {
                        e.choice.insert("target".to_string(), Value::Null);
                    }
                } else {
                    let default_t = candidates
                        .first()
                        .map(|c| Value::from(c.uid))
                        .unwrap_or(Value::Null);
                    e.choice.insert("target".to_string(), default_t);
                }
            }
        }
    }

    pub fn viable_modes(
        &self,
        c: &Card,
        modes: &[String],
        pending: Option<&Effect>,
    ) -> Vec<String> {
        let mut result = Vec::new();
        for mode in modes {
            let valid = if mode.starts_with("draw") || mode == "select" {
                !self.p[c.owner].deck.is_empty()
            } else if mode.starts_with("break") {
                !self.p[1 - c.owner].shields.is_empty()
            } else if mode.starts_with("discard_enemy") {
                !self.p[1 - c.owner].hand.is_empty()
            } else if mode.starts_with("recover") {
                self.p[c.owner].discard.iter().any(|t| match mode.as_str() {
                    "recover_character" => t.card_type() == "Character",
                    "recover_action" => t.card_type() == "Action" || t.card_type() == "Counter",
                    _ => true,
                })
            } else if mode.starts_with("bounce") {
                let dummy = Effect::new(
                    c.clone(),
                    "entry",
                    Vec::new(),
                    Map::new(),
                    pending.map(|p| Box::new(p.clone())),
                );
                !self.bounce_candidates(&dummy, mode).is_empty()
            } else {
                false
            };
            if valid {
                result.push(mode.clone());
            }
        }
        result
    }

    fn select_card(cards: &[Card], choice: &Map<String, Value>, key: &str) -> Option<Card> {
        if let Some(val) = choice.get(key) {
            let want_uid = as_strict_i32(val);
            cards.iter().find(|c| Some(c.uid) == want_uid).cloned()
        } else {
            cards.first().cloned()
        }
    }

    fn trace_decision(&mut self, e: &Effect) {
        if self.trace_transitions {
            self.attempt_decisions.push(json!({
                "source": e.source.uid,
                "card_id": e.source.id(),
                "kind": e.kind,
                "choice": e.choice,
                "ops": e.ops
            }));
        }
    }

    fn resolve<P: DecisionProviders>(
        &mut self,
        e: &mut Effect,
        providers: &mut P,
    ) -> Result<(), Illegal> {
        if self.sim_mode {
            // SimGame delayed `select` and `discard` resolution hooks
            if let Some(sel_op) = e
                .ops
                .iter()
                .find(|o| o.get("kind").and_then(|v| v.as_str()) == Some("select"))
            {
                let n = sel_op.get("n").and_then(as_strict_i32).unwrap_or(0) as usize;
                let top_cards: Vec<Card> = self.p[e.source.owner]
                    .deck
                    .iter()
                    .take(n)
                    .cloned()
                    .collect();
                if !top_cards.is_empty() {
                    let picked = if providers.has_pick() {
                        let pk = providers.pick(self, e.source.owner, &top_cards)?;
                        if self.trace_transitions {
                            self.provider_outputs.push(json!({
                                "role": "pick",
                                "output": pk
                            }));
                        }
                        pk
                    } else {
                        top_cards
                            .iter()
                            .max_by(|a, b| {
                                crate::policy::card_value(a)
                                    .partial_cmp(&crate::policy::card_value(b))
                                    .unwrap_or(std::cmp::Ordering::Equal)
                                    .then_with(|| b.uid.cmp(&a.uid))
                            })
                            .unwrap()
                            .uid
                    };
                    e.choice.insert("pick".to_string(), json!(picked));
                }
            }
            let ops_snapshot = e.ops.clone();
            for op in &ops_snapshot {
                let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                if k == "discard" || k == "discard_enemy" {
                    let owner = if k == "discard" {
                        e.source.owner
                    } else {
                        1 - e.source.owner
                    };
                    let n = op.get("n").and_then(as_strict_i32).unwrap_or(1) as usize;
                    let disc_uids = if providers.has_discard() {
                        let d = providers.discard(self, owner, n)?;
                        if self.trace_transitions {
                            self.provider_outputs.push(json!({
                                "role": "discard",
                                "output": d
                            }));
                        }
                        d
                    } else {
                        let mut sorted_hand = self.p[owner].hand.clone();
                        sorted_hand.sort_by(|a, b| {
                            crate::policy::card_value(a)
                                .partial_cmp(&crate::policy::card_value(b))
                                .unwrap_or(std::cmp::Ordering::Equal)
                                .then_with(|| a.uid.cmp(&b.uid))
                        });
                        sorted_hand.into_iter().take(n).map(|c| c.uid).collect()
                    };
                    e.choice.insert(
                        "discard".to_string(),
                        Value::Array(disc_uids.into_iter().map(|u| json!(u)).collect()),
                    );
                }
            }
        }

        validate_ops(&e.ops)?;
        validate_choice(&e.choice, true)?;
        self.log
            .push(vec![json!("resolve"), json!(e.source.uid), json!(e.kind)]);
        let owner = e.source.owner;
        let modal = e
            .choice
            .get("_options")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        let default_opt = e
            .choice
            .get("_options")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(Value::Null);
        let mode_val = e
            .choice
            .get("mode")
            .cloned()
            .unwrap_or_else(|| default_opt.clone());
        let effect_log_start = self.log.len();
        let shields_before = self.p[1 - owner].shields.len() as i32;
        if modal {
            self.log.push(vec![
                json!("mode_resolved"),
                json!(e.source.uid),
                json!(e.source.id()),
                json!(owner),
                json!(e.kind),
                mode_val.clone(),
            ]);
        }

        let mut resolved_ops = Vec::new();
        for original in &e.ops {
            let k = original.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if k == "break_finisher" {
                let threshold = original
                    .get("threshold")
                    .and_then(as_strict_i32)
                    .unwrap_or(0) as usize;
                let amount = if self.p[1 - owner].shields.len() <= threshold {
                    original.get("finish").and_then(as_strict_i32).unwrap_or(0)
                } else {
                    original.get("n").and_then(as_strict_i32).unwrap_or(0)
                };
                self.log.push(vec![
                    json!("finisher"),
                    json!(e.source.uid),
                    json!(amount),
                    json!(self.p[1 - owner].shields.len()),
                ]);
                resolved_ops.push(json!({"kind": "break", "n": amount}));
            } else {
                resolved_ops.push(original.clone());
            }
        }
        if e.source.id() == "H3W-AL-017" && e.kind == "combo" {
            let qm = e.choice.get("mode").cloned().unwrap_or(Value::Null);
            self.log
                .push(vec![json!("queen_mode"), json!(e.source.uid), qm]);
        }

        for op in &resolved_ops {
            if self.winner.is_some() {
                break;
            }
            let k = op.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let optional = op
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if optional && e.choice.get("mode").and_then(|v| v.as_str()) == Some("skip") {
                continue;
            }
            match k {
                "require_characters_in_sequence" => {
                    let req_n = op.get("n").and_then(as_strict_i32).unwrap_or(0);
                    let chars = self
                        .sequence
                        .iter()
                        .filter(|c| {
                            self.present(c.uid) && c.owner == owner && c.card_type() == "Character"
                        })
                        .count() as i32;
                    if chars < req_n {
                        break;
                    }
                }
                "draw" => {
                    let n = op.get("n").and_then(as_strict_i32).unwrap_or(0);
                    self.draw_internal(owner, n)?;
                }
                "discard" | "discard_enemy" => {
                    let p_idx = if k == "discard" { owner } else { 1 - owner };
                    let n = op.get("n").and_then(as_strict_i32).unwrap_or(1) as usize;
                    let selected: Vec<i32> = e
                        .choice
                        .get("discard")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(as_strict_i32).collect())
                        .unwrap_or_default();
                    let max_disc = n.min(self.p[p_idx].hand.len());
                    if selected.len() > max_disc
                        || selected
                            .iter()
                            .any(|uid| !self.p[p_idx].hand.iter().any(|c| c.uid == *uid))
                    {
                        return Err(Illegal("discard target must be in affected hand".into()));
                    }
                    for i in 0..n {
                        let hand_clone = self.p[p_idx].hand.clone();
                        let mut temp_choice = Map::new();
                        if i < selected.len() {
                            temp_choice.insert("target".to_string(), json!(selected[i]));
                        }
                        if let Some(c) = Self::select_card(&hand_clone, &temp_choice, "target") {
                            self.p[p_idx].hand.retain(|x| x.uid != c.uid);
                            self.p[p_idx].discard.push(c.clone());
                            self.log.push(vec![json!("discard"), json!(c.uid)]);
                        }
                    }
                }
                _ if is_bounce(k) || k == "counter_bounce_later" => {
                    let cands = self.bounce_candidates(e, k);
                    if let Some(c) = Self::select_card(&cands, &e.choice, "target") {
                        self.p[c.owner].field.retain(|x| x.uid != c.uid);
                        self.p[c.owner].hand.push(c.clone());
                        self.log.push(vec![json!("return"), json!(c.uid)]);
                    }
                }
                "recover_character" | "recover_action" | "recover_any" => {
                    let cands: Vec<Card> = self.p[owner]
                        .discard
                        .iter()
                        .filter(|c| match k {
                            "recover_character" => c.card_type() == "Character",
                            "recover_action" => {
                                c.card_type() == "Action" || c.card_type() == "Counter"
                            }
                            _ => true,
                        })
                        .cloned()
                        .collect();
                    if let Some(c) = Self::select_card(&cands, &e.choice, "target") {
                        self.p[owner].discard.retain(|x| x.uid != c.uid);
                        self.p[owner].hand.push(c.clone());
                        self.log.push(vec![json!("recover"), json!(c.uid)]);
                    }
                }
                "select" => {
                    let n = op.get("n").and_then(as_strict_i32).unwrap_or(0) as usize;
                    let top: Vec<Card> = self.p[owner].deck.iter().take(n).cloned().collect();
                    if let Some(c) = Self::select_card(&top, &e.choice, "pick") {
                        let top_len = top.len();
                        self.p[owner].deck.drain(0..top_len);
                        self.p[owner].hand.push(c.clone());
                        for x in top {
                            if x.uid != c.uid {
                                self.p[owner].deck.push(x);
                            }
                        }
                        self.log.push(vec![json!("select"), json!(c.uid)]);
                    }
                }
                "break" => {
                    let n = op.get("n").and_then(as_strict_i32).unwrap_or(0);
                    self.break_shields_internal(owner, n)?;
                }
                "cancel" => {
                    let pending = e
                        .target_effect
                        .as_mut()
                        .ok_or_else(|| Illegal("cancel requires pending effect".into()))?;
                    pending.cancelled = true;
                    pending.source.cancelled = true;
                    let p_uid = pending.source.uid;
                    let p_kind = pending.kind.clone();
                    let mut sc = self.get_card(p_uid)?;
                    sc.cancelled = true;
                    self.sync_card(&sc);
                    self.log
                        .push(vec![json!("cancel"), json!(p_uid), json!(p_kind)]);
                }
                _ => return Err(Illegal(format!("unsupported op: {k}"))),
            }
        }

        if modal {
            let events = &self.log[effect_log_start..];
            let breaks = shields_before - (self.p[1 - owner].shields.len() as i32);
            let draws = events
                .iter()
                .filter(|x| {
                    x.first().and_then(|v| v.as_str()) == Some("draw")
                        && x.get(1).and_then(|v| v.as_u64()) == Some(owner as u64)
                })
                .count() as i32;
            let recoveries = events
                .iter()
                .filter(|x| x.first().and_then(|v| v.as_str()) == Some("recover"))
                .count() as i32;
            let enemy_discards = events
                .iter()
                .filter(|x| {
                    if x.first().and_then(|v| v.as_str()) == Some("discard") {
                        if let Some(uid) = x.get(1).and_then(as_strict_i32) {
                            if let Some(c) = self.cards.get(&uid) {
                                return c.owner != owner;
                            }
                        }
                    }
                    false
                })
                .count() as i32;
            self.log.push(vec![
                json!("mode_outcome"),
                json!(e.source.uid),
                json!(e.source.id()),
                json!(owner),
                mode_val,
                json!(breaks),
                json!(draws),
                json!(recoveries),
                json!(enemy_discards),
            ]);
        }
        Ok(())
    }

    pub fn draw(&mut self, owner: usize, n: i32) -> Result<(), Illegal> {
        let args = json!([owner, n]);
        self.execute_command("draw", args, json!({}), Vec::new(), |g| {
            g.draw_internal(owner, n)
        })
    }

    fn draw_internal(&mut self, owner: usize, n: i32) -> Result<(), Illegal> {
        if self.winner.is_some() {
            return Err(Illegal("cannot draw after game end".into()));
        }
        if owner > 1 || n < 0 {
            return Err(Illegal("invalid draw".into()));
        }
        self.validation_passed = true;
        for _ in 0..n {
            if !self.p[owner].deck.is_empty() {
                let c = self.p[owner].deck.remove(0);
                let uid = c.uid;
                self.p[owner].hand.push(c);
                self.log.push(vec![json!("draw"), json!(owner), json!(uid)]);
            }
        }
        Ok(())
    }

    pub fn break_shields(&mut self, owner: usize, n: i32) -> Result<(), Illegal> {
        let args = json!([owner, n]);
        self.execute_command("break_shields", args, json!({}), Vec::new(), |g| {
            g.break_shields_internal(owner, n)
        })
    }

    fn break_shields_internal(&mut self, owner: usize, n: i32) -> Result<(), Illegal> {
        if owner > 1 || n < 0 {
            return Err(Illegal("invalid shield break".into()));
        }
        self.validation_passed = true;
        for _ in 0..n {
            if self.winner.is_some() {
                break;
            }
            let def = 1 - owner;
            if self.opening_protection
                && self.turn_number == 1
                && Some(owner) == self.active
                && self.p[def].shields.len() == 1
            {
                self.log.push(vec![json!("opening_floor"), json!(owner)]);
                break;
            }
            if self.p[def].shields.is_empty() {
                return Err(Illegal("invalid zero-shield live state".into()));
            }
            let c = self.p[def].shields[0].clone();
            self.log.push(vec![json!("reveal"), json!(c.uid)]);
            if c.shield_trigger().is_some() {
                self.draw_internal(def, 1)?;
            }
            let popped = self.p[def].shields.remove(0);
            self.p[def].discard.push(popped);
            self.log.push(vec![json!("shield_discard"), json!(c.uid)]);
            if self.p[def].shields.is_empty() {
                self.winner = Some(owner);
                self.log.push(vec![json!("win"), json!(owner)]);
            }
        }
        Ok(())
    }

    pub fn end_turn(&mut self, discard_uids: &[i32]) -> Result<(), Illegal> {
        self.end_turn_with_providers(discard_uids, &NoProviders)
    }

    pub fn end_turn_with_providers<P: DecisionProviders>(
        &mut self,
        discard_uids: &[i32],
        providers: &P,
    ) -> Result<(), Illegal> {
        let roles = Self::provider_roles(providers);
        let args = json!([discard_uids]);
        self.execute_command("end_turn", args, json!({}), roles, |g| {
            if g.phase != "resolved" {
                return Err(Illegal("resolve before end turn".into()));
            }
            let act = g.active.ok_or_else(|| Illegal("no active player".into()))?;
            let hand_len = g.p[act].hand.len();
            let excess = hand_len.saturating_sub(6);
            if g.winner.is_some() {
                g.validation_passed = true;
                g.phase = "ended".to_string();
                return Ok(());
            }
            let unique: HashSet<i32> = discard_uids.iter().copied().collect();
            let all_in_hand = discard_uids
                .iter()
                .all(|uid| g.p[act].hand.iter().any(|c| c.uid == *uid));
            if discard_uids.len() != excess || unique.len() != excess || !all_in_hand {
                return Err(Illegal(
                    "choose exactly the excess hand cards to discard to six".into(),
                ));
            }
            g.validation_passed = true;
            for &uid in discard_uids {
                let c = g.get_card(uid)?;
                g.p[act].hand.retain(|x| x.uid != uid);
                g.p[act].discard.push(c);
                g.log.push(vec![json!("end_discard"), json!(uid)]);
            }
            g.phase = "ended".to_string();
            g.invariants()?;
            Ok(())
        })
    }

    pub fn invariants(&self) -> Result<(), Illegal> {
        let mut ids = Vec::new();
        for (owner, p) in self.p.iter().enumerate() {
            if p.actions < 0 || p.actions > 6 {
                return Err(Illegal("actions out of 0..6".into()));
            }
            for zone in ["hand", "deck", "field", "shields", "discard"] {
                for c in p.zone(zone) {
                    if c.owner != owner {
                        return Err(Illegal("card owner mismatch".into()));
                    }
                    ids.push(c.uid);
                }
            }
        }
        let unique: HashSet<i32> = ids.iter().copied().collect();
        if ids.len() != unique.len() {
            return Err(Illegal("one physical card in more than one zone".into()));
        }
        let card_keys: HashSet<i32> = self.cards.keys().copied().collect();
        if unique != card_keys {
            return Err(Illegal("card conservation".into()));
        }
        if let Some(w) = self.winner {
            if !self.p[1 - w].shields.is_empty() {
                return Err(Illegal("winner inconsistent with shields".into()));
            }
        }
        Ok(())
    }
}
