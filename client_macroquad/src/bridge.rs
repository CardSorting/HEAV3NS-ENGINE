use heav3ns_engine::{
    adapter::{choices_for, stock_game},
    engine::{load_proposed_specs, Card, DecisionProviders, Effect, Game, Illegal, Response},
    policy::{card_value, AuditPolicy},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, VecDeque};

/// Discrete player commands sent from the Macroquad client to the headless backend (`Client -> Backend`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameCommand {
    /// Play a card from hand (into the active sequence or with an explicit target / slot).
    PlayCard {
        card_id: i32,
        target_id: Option<i32>,
    },
    /// Attack / direct a unit or sequence strike toward a defender or opponent shield row.
    Attack {
        attacker_id: i32,
        defender_id: i32,
    },
    /// Resolve the current open sequence (`close`).
    ResolveSequence,
    /// Open a new sequence when in `resolved` phase (`begin_sequence`).
    BeginSequence,
    /// End the current player's turn.
    EndTurn,
    /// Submit a modal choice, target, counter response, pick, or discard selection during an interactive prompt.
    SubmitDecision {
        option_index: Option<usize>,
        selected_uids: Vec<i32>,
    },
    /// Trigger an interactive corner prop on the battlefield.
    TriggerProp { prop_index: usize },
    /// Reset the match with a new seed and starter.
    ResetMatch { seed: u64, starter: usize },
}

/// Ordered presentation events emitted by the backend (`Backend -> Client`) to drive 60+ FPS animations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VisualEvent {
    TurnStarted {
        seat: usize,
        turn_number: usize,
    },
    CardDrawn {
        seat: usize,
        card_uid: Option<i32>,
        card_spec_id: String,
        title: String,
    },
    ManaSpent {
        seat: usize,
        amount: i32,
        remaining: i32,
    },
    UnitSummoned {
        seat: usize,
        card_uid: i32,
        card_spec_id: String,
        title: String,
        cost: i32,
        card_type: String,
        slot_index: usize,
    },
    SpellCast {
        seat: usize,
        card_uid: i32,
        card_spec_id: String,
        title: String,
        target_uid: Option<i32>,
    },
    DamageDealt {
        attacker_uid: Option<i32>,
        target_seat: usize,
        target_uid: Option<i32>,
        amount: i32,
        is_shield_break: bool,
    },
    ShieldRevealed {
        seat: usize,
        card_uid: i32,
        card_spec_id: String,
        title: String,
        trigger_text: Option<String>,
    },
    UnitBounced {
        seat: usize,
        card_uid: i32,
        title: String,
    },
    CardRecovered {
        seat: usize,
        card_uid: i32,
        title: String,
    },
    UnitDestroyed {
        seat: usize,
        card_uid: i32,
        title: String,
        reason: String,
    },
    EffectCancelled {
        card_uid: i32,
        title: String,
        kind: String,
    },
    ComboLostSupport {
        card_uid: i32,
        title: String,
    },
    MatchWon {
        winner_seat: usize,
    },
    BannerMessage {
        text: String,
        subtext: String,
    },
}

impl VisualEvent {
    /// Returns true if this event is a blocking animation (e.g., fireball / spell projectile,
    /// unit summon slam, shield break impact, turn banner) that holds the `AnimationSequencer` queue
    /// until completion, or false if it runs concurrently (e.g., floating damage numbers, mana ticks).
    pub fn is_blocking(&self) -> bool {
        match self {
            VisualEvent::TurnStarted { .. } => true,
            VisualEvent::CardDrawn { .. } => true,
            VisualEvent::ManaSpent { .. } => false,
            VisualEvent::UnitSummoned { .. } => true,
            VisualEvent::SpellCast { .. } => true,
            VisualEvent::DamageDealt {
                is_shield_break, ..
            } => *is_shield_break,
            VisualEvent::ShieldRevealed { .. } => true,
            VisualEvent::UnitBounced { .. } => true,
            VisualEvent::CardRecovered { .. } => false,
            VisualEvent::UnitDestroyed { .. } => false,
            VisualEvent::EffectCancelled { .. } => true,
            VisualEvent::ComboLostSupport { .. } => false,
            VisualEvent::MatchWon { .. } => true,
            VisualEvent::BannerMessage { .. } => false,
        }
    }

    /// Default duration in seconds for this visual event in the sequencer.
    pub fn duration(&self) -> f32 {
        match self {
            VisualEvent::TurnStarted { .. } => 0.55,
            VisualEvent::CardDrawn { .. } => 0.22,
            VisualEvent::ManaSpent { .. } => 0.25,
            VisualEvent::UnitSummoned { .. } => 0.38,
            VisualEvent::SpellCast { .. } => 0.45,
            VisualEvent::DamageDealt { .. } => 0.42,
            VisualEvent::ShieldRevealed { .. } => 0.50,
            VisualEvent::UnitBounced { .. } => 0.32,
            VisualEvent::CardRecovered { .. } => 0.30,
            VisualEvent::UnitDestroyed { .. } => 0.35,
            VisualEvent::EffectCancelled { .. } => 0.40,
            VisualEvent::ComboLostSupport { .. } => 0.35,
            VisualEvent::MatchWon { .. } => 0.80,
            VisualEvent::BannerMessage { .. } => 0.60,
        }
    }
}

/// Snapshot of a card's visual metadata for the client's `VisualBoardState`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualCardData {
    pub uid: i32,
    pub spec_id: String,
    pub title: String,
    pub card_type: String,
    pub cost: i32,
    pub text: String,
    pub combo_n: Option<i32>,
    pub shield_trigger: Option<String>,
    pub owner: usize,
    pub attack_badge: i32,
    pub health_badge: i32,
    pub has_taunt: bool,
    pub has_shield: bool,
    pub is_playable: bool,
}

/// Normalizes Unicode typography (curly quotes, em-dashes, ellipses, bullets) into crisp ASCII
/// equivalents so Macroquad's default bitmap font renders all card titles and rules text cleanly.
pub fn sanitize_ascii_typography(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '’' | '‘' => out.push('\''),
            '“' | '”' => out.push('"'),
            '—' | '–' => out.push_str(" - "),
            '…' => out.push_str("..."),
            '·' | '•' => out.push('-'),
            '→' => out.push_str("->"),
            _ => out.push(ch),
        }
    }
    out
}

impl VisualCardData {
    pub fn from_engine_card(c: &Card, is_playable: bool) -> Self {
        // Derive attack/health visual badges deterministically from card cost & ops so units on the board
        // display rich combat badges, taunt frames, and shield indicators.
        let ops_str = c.spec.to_string();
        let break_bonus = if ops_str.contains("break_finisher") {
            3
        } else if ops_str.contains("break2") || ops_str.contains("\"n\":2") {
            2
        } else {
            1
        };
        let attack_badge = (c.cost() + break_bonus).max(1);
        let health_badge = (c.cost() * 2 + if c.combo_n().is_some() { 1 } else { 0 }).max(1);
        let has_taunt = c.card_type() == "Character"
            && (c.cost() >= 3 || ops_str.contains("bounce") || ops_str.contains("cancel"));
        let has_shield = c.shield_trigger().is_some() || c.response_play();

        Self {
            uid: c.uid,
            spec_id: c.id().to_string(),
            title: sanitize_ascii_typography(c.title()),
            card_type: c.card_type().to_string(),
            cost: c.cost(),
            text: sanitize_ascii_typography(c.text()),
            combo_n: c.combo_n(),
            shield_trigger: c.shield_trigger().map(sanitize_ascii_typography),
            owner: c.owner,
            attack_badge,
            health_badge,
            has_taunt,
            has_shield,
            is_playable,
        }
    }
}

/// Interactive prompt option when a card or response requires a player decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptOption {
    pub index: usize,
    pub option_id: String,
    pub label: String,
    pub target_uid: Option<i32>,
    pub mode: Option<String>,
}

/// Active interactive prompt presented to the player if a command requires a follow-up choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivePrompt {
    pub decision_id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub options: Vec<PromptOption>,
    pub required_cards: usize,
    pub candidate_cards: Vec<VisualCardData>,
}

/// The client's local presentation state (`VisualBoardState`), updated incrementally as `VisualEvent`s
/// are popped and animated by `AnimationSequencer`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualBoardState {
    pub seed: u64,
    pub turn_number: usize,
    pub active_seat: usize,
    pub phase: String,
    pub mana: [i32; 2],
    pub max_mana: [i32; 2],
    pub shields_count: [usize; 2],
    pub deck_count: [usize; 2],
    pub discard_count: [usize; 2],
    pub opponent_hand_count: usize,
    pub player_hand: Vec<VisualCardData>,
    pub board_units: [Vec<VisualCardData>; 2],
    pub active_sequence: Vec<VisualCardData>,
    pub winner: Option<usize>,
    pub status_line: String,
    pub prompt: Option<ActivePrompt>,
    pub combat_log: Vec<String>,
}

impl Default for VisualBoardState {
    fn default() -> Self {
        Self {
            seed: 42,
            turn_number: 1,
            active_seat: 0,
            phase: "placement".to_string(),
            mana: [6, 6],
            max_mana: [6, 6],
            shields_count: [6, 6],
            deck_count: [29, 29],
            discard_count: [0, 0],
            opponent_hand_count: 5,
            player_hand: Vec::new(),
            board_units: [Vec::new(), Vec::new()],
            active_sequence: Vec::new(),
            winner: None,
            status_line: "Alice's Turn — Drag cards to the Enchanted Table or click Resolve.".to_string(),
            prompt: None,
            combat_log: Vec::new(),
        }
    }
}

/// Active animation item running inside `AnimationSequencer`.
#[derive(Clone, Debug)]
pub struct RunningAnimation {
    pub event: VisualEvent,
    pub elapsed: f32,
    pub duration: f32,
    pub blocking: bool,
    /// Snapshot of the target `VisualBoardState` to apply when this event finishes (or starts).
    pub state_after: Option<VisualBoardState>,
}

impl RunningAnimation {
    pub fn progress(&self) -> f32 {
        if self.duration <= 0.0001 {
            1.0
        } else {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        }
    }
}

/// `AnimationSequencer` drains `VecDeque<VisualEvent>` emitted by the backend.
/// Blocking animations hold the queue until completion; non-blocking animations run concurrently.
pub struct AnimationSequencer {
    pub queue: VecDeque<(VisualEvent, Option<VisualBoardState>)>,
    pub active_blocking: Option<RunningAnimation>,
    pub active_concurrent: Vec<RunningAnimation>,
    pub completed_this_frame: Vec<VisualEvent>,
}

impl Default for AnimationSequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationSequencer {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            active_blocking: None,
            active_concurrent: Vec::new(),
            completed_this_frame: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, event: VisualEvent, state_after: Option<VisualBoardState>) {
        self.queue.push_back((event, state_after));
    }

    pub fn enqueue_batch(
        &mut self,
        events: Vec<VisualEvent>,
        final_state: VisualBoardState,
    ) {
        if events.is_empty() {
            self.queue.push_back((
                VisualEvent::BannerMessage {
                    text: String::new(),
                    subtext: String::new(),
                },
                Some(final_state),
            ));
            return;
        }
        let len = events.len();
        for (i, ev) in events.into_iter().enumerate() {
            let st = if i + 1 == len {
                Some(final_state.clone())
            } else {
                None
            };
            self.queue.push_back((ev, st));
        }
    }

    pub fn is_busy(&self) -> bool {
        self.active_blocking.is_some() || !self.queue.is_empty()
    }

    /// Advances active animations by `dt` seconds and pops new events from the queue,
    /// updating `board_state` only as events are popped and animated.
    /// Returns any newly started `VisualEvent`s so the VFX/Audio systems can trigger particles/SFX.
    pub fn update(&mut self, dt: f32, board_state: &mut VisualBoardState) -> Vec<VisualEvent> {
        self.completed_this_frame.clear();
        let mut started = Vec::new();

        // 1. Update concurrent non-blocking animations
        let mut i = 0;
        while i < self.active_concurrent.len() {
            self.active_concurrent[i].elapsed += dt;
            if self.active_concurrent[i].elapsed >= self.active_concurrent[i].duration {
                let finished = self.active_concurrent.remove(i);
                if let Some(st) = finished.state_after {
                    *board_state = st;
                }
                self.completed_this_frame.push(finished.event);
            } else {
                i += 1;
            }
        }

        // 2. Update active blocking animation if present
        if let Some(ref mut blk) = self.active_blocking {
            blk.elapsed += dt;
            if blk.elapsed >= blk.duration {
                let finished = self.active_blocking.take().unwrap();
                if let Some(st) = finished.state_after {
                    *board_state = st;
                }
                self.completed_this_frame.push(finished.event);
            } else {
                // Still holding the queue!
                return started;
            }
        }

        // 3. Pop from queue until we hit a blocking animation
        while let Some((ev, st_opt)) = self.queue.pop_front() {
            // Apply incremental visual changes immediately when popped so the card/unit appears for its entrance animation
            Self::apply_incremental_event(board_state, &ev);
            let dur = ev.duration();
            let blocking = ev.is_blocking();
            started.push(ev.clone());

            if dur <= 0.001 {
                if let Some(st) = st_opt {
                    *board_state = st;
                }
                self.completed_this_frame.push(ev);
                continue;
            }

            let anim = RunningAnimation {
                event: ev,
                elapsed: 0.0,
                duration: dur,
                blocking,
                state_after: st_opt,
            };

            if blocking {
                self.active_blocking = Some(anim);
                break;
            } else {
                self.active_concurrent.push(anim);
            }
        }

        started
    }

    fn apply_incremental_event(board_state: &mut VisualBoardState, ev: &VisualEvent) {
        match ev {
            VisualEvent::TurnStarted { seat, turn_number } => {
                board_state.active_seat = *seat;
                board_state.turn_number = *turn_number;
                board_state.mana[*seat] = 6;
                board_state.combat_log.push(format!(
                    "Turn {}: {} begins.",
                    turn_number,
                    if *seat == 0 { "Alice" } else { "White Rabbit" }
                ));
            }
            VisualEvent::ManaSpent {
                seat,
                amount,
                remaining,
            } => {
                board_state.mana[*seat] = *remaining;
                if *amount > 0 {
                    board_state.combat_log.push(format!(
                        "{} spent {} Actions ({} remaining).",
                        if *seat == 0 { "Alice" } else { "White Rabbit" },
                        amount,
                        remaining
                    ));
                }
            }
            VisualEvent::CardDrawn {
                seat,
                title,
                ..
            } => {
                board_state.deck_count[*seat] = board_state.deck_count[*seat].saturating_sub(1);
                if *seat == 1 {
                    board_state.opponent_hand_count += 1;
                }
                board_state.combat_log.push(format!(
                    "{} drew {}.",
                    if *seat == 0 { "Alice" } else { "White Rabbit" },
                    if *seat == 0 { title.as_str() } else { "a card" }
                ));
            }
            VisualEvent::UnitSummoned {
                seat,
                card_uid,
                title,
                ..
            } => {
                if *seat == 0 {
                    if let Some(pos) = board_state
                        .player_hand
                        .iter()
                        .position(|c| c.uid == *card_uid)
                    {
                        let card = board_state.player_hand.remove(pos);
                        board_state.board_units[0].push(card.clone());
                        board_state.active_sequence.push(card);
                    }
                } else {
                    board_state.opponent_hand_count =
                        board_state.opponent_hand_count.saturating_sub(1);
                }
                board_state.combat_log.push(format!(
                    "{} played {} to the board.",
                    if *seat == 0 { "Alice" } else { "White Rabbit" },
                    title
                ));
            }
            VisualEvent::SpellCast {
                seat,
                card_uid,
                title,
                ..
            } => {
                if *seat == 0 {
                    if let Some(pos) = board_state
                        .player_hand
                        .iter()
                        .position(|c| c.uid == *card_uid)
                    {
                        let card = board_state.player_hand.remove(pos);
                        board_state.active_sequence.push(card);
                    }
                } else {
                    board_state.opponent_hand_count =
                        board_state.opponent_hand_count.saturating_sub(1);
                }
                board_state.combat_log.push(format!(
                    "{} cast {}!",
                    if *seat == 0 { "Alice" } else { "White Rabbit" },
                    title
                ));
            }
            VisualEvent::DamageDealt {
                target_seat,
                amount,
                is_shield_break,
                ..
            } => {
                if *is_shield_break {
                    board_state.shields_count[*target_seat] = board_state.shields_count
                        [*target_seat]
                        .saturating_sub(*amount as usize);
                }
            }
            VisualEvent::ShieldRevealed {
                seat,
                title,
                trigger_text,
                ..
            } => {
                let trig = trigger_text
                    .as_deref()
                    .map(|t| format!(" (Trigger: {t})"))
                    .unwrap_or_default();
                board_state.combat_log.push(format!(
                    "{}'s Shield revealed: {}{}",
                    if *seat == 0 { "Alice" } else { "White Rabbit" },
                    title,
                    trig
                ));
            }
            VisualEvent::UnitBounced {
                seat,
                card_uid,
                title,
            } => {
                board_state.board_units[*seat].retain(|u| u.uid != *card_uid);
                board_state.combat_log.push(format!(
                    "{} returned to {}'s hand.",
                    title,
                    if *seat == 0 { "Alice" } else { "White Rabbit" }
                ));
            }
            VisualEvent::UnitDestroyed {
                seat,
                card_uid,
                title,
                reason,
            } => {
                board_state.board_units[*seat].retain(|u| u.uid != *card_uid);
                board_state.discard_count[*seat] += 1;
                board_state
                    .combat_log
                    .push(format!("{title} ({reason})."));
            }
            VisualEvent::EffectCancelled { title, kind, .. } => {
                board_state
                    .combat_log
                    .push(format!("{title}: {kind} cancelled!"));
            }
            VisualEvent::MatchWon { winner_seat } => {
                board_state.winner = Some(*winner_seat);
                board_state.combat_log.push(format!(
                    "{} wins the match!",
                    if *winner_seat == 0 {
                        "Alice"
                    } else {
                        "White Rabbit"
                    }
                ));
            }
            _ => {}
        }
        if board_state.combat_log.len() > 24 {
            let excess = board_state.combat_log.len() - 24;
            board_state.combat_log.drain(0..excess);
        }
    }
}

/// Bridge providers that use player-supplied target hints when available and fall back to smart
/// deterministic choices so gameplay flows seamlessly from drag-and-drop and targeting arrows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordedDecision {
    OptionIndex(usize),
    SelectedCards(Vec<i32>),
}

/// Bridge providers that replay already-answered interactive prompts for the current command,
/// use player-supplied drag/arrow target hints when unambiguous, and pause with `PROMPT_REQUIRED`
/// when an interactive modal choice, Counter response, deck pick, or discard selection is needed.
struct BridgeDecisionProvider {
    ai: AuditPolicy,
    preferred_target: Option<i32>,
    preferred_mode: Option<String>,
    recorded: Vec<RecordedDecision>,
    cursor: usize,
    interactive_prompts: bool,
    captured_prompt: Option<ActivePrompt>,
}

impl BridgeDecisionProvider {
    fn new(
        preferred_target: Option<i32>,
        preferred_mode: Option<String>,
        recorded: Vec<RecordedDecision>,
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

    fn next_recorded(&mut self) -> Option<RecordedDecision> {
        if self.cursor < self.recorded.len() {
            let d = self.recorded[self.cursor].clone();
            self.cursor += 1;
            Some(d)
        } else {
            None
        }
    }
}

impl DecisionProviders for BridgeDecisionProvider {
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
            if let Some(RecordedDecision::OptionIndex(idx)) = self.next_recorded() {
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
                let options: Vec<PromptOption> = raw_opts
                    .iter()
                    .enumerate()
                    .map(|(idx, (v, m))| PromptOption {
                        index: idx,
                        option_id: idx.to_string(),
                        label: sanitize_ascii_typography(
                            v.get("label")
                                .and_then(|s| s.as_str())
                                .unwrap_or("Choose option"),
                        ),
                        target_uid: m.get("target").and_then(|t| t.as_i64()).map(|u| u as i32),
                        mode: m
                            .get("mode")
                            .and_then(|md| md.as_str())
                            .map(|s| s.to_string()),
                    })
                    .collect();
                self.captured_prompt = Some(ActivePrompt {
                    decision_id: format!("choice:{}:{}:{}", c.uid, kind, self.cursor),
                    kind: "choice".to_string(),
                    title: sanitize_ascii_typography(&format!("Choose {}: {}", c.title(), kind)),
                    subtitle: "Select a mode or target on the Enchanted Table.".to_string(),
                    options,
                    required_cards: 0,
                    candidate_cards: Vec::new(),
                });
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
            if let Some(RecordedDecision::OptionIndex(idx)) = self.next_recorded() {
                if idx == 0 {
                    return Ok(None);
                }
                if let Some(c) = valid_counters.get(idx - 1) {
                    return Ok(Some(vec![Response::new(c.uid)]));
                }
            }
            if self.interactive_prompts {
                let mut options = Vec::with_capacity(valid_counters.len() + 1);
                options.push(PromptOption {
                    index: 0,
                    option_id: "0".to_string(),
                    label: "Pass this response window".to_string(),
                    target_uid: None,
                    mode: Some("pass".to_string()),
                });
                for (i, c) in valid_counters.iter().enumerate() {
                    options.push(PromptOption {
                        index: i + 1,
                        option_id: (i + 1).to_string(),
                        label: sanitize_ascii_typography(&format!(
                            "Play {} ({} Action{})",
                            c.title(),
                            c.cost(),
                            if c.cost() == 1 { "" } else { "s" }
                        )),
                        target_uid: Some(c.uid),
                        mode: Some("respond".to_string()),
                    });
                }
                self.captured_prompt = Some(ActivePrompt {
                    decision_id: format!(
                        "response:{}:{}:{}",
                        pending.source.uid, pending.kind, self.cursor
                    ),
                    kind: "response".to_string(),
                    title: sanitize_ascii_typography(&format!(
                        "Respond to {}: {}",
                        pending.source.title(),
                        pending.kind
                    )),
                    subtitle: "Choose a Counter/Response from hand or Pass priority.".to_string(),
                    options,
                    required_cards: 0,
                    candidate_cards: valid_counters
                        .iter()
                        .map(|c| VisualCardData::from_engine_card(c, true))
                        .collect(),
                });
                return Err(Illegal("PROMPT_REQUIRED".into()));
            }
        }
        self.ai.respond(g, pending, is_reply)
    }

    fn pick(&mut self, g: &Game, owner: usize, cards: &[Card]) -> Result<i32, Illegal> {
        if owner == 0 && cards.len() > 1 {
            if let Some(dec) = self.next_recorded() {
                match dec {
                    RecordedDecision::OptionIndex(idx) => {
                        if let Some(c) = cards.get(idx) {
                            return Ok(c.uid);
                        }
                    }
                    RecordedDecision::SelectedCards(uids) => {
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
                let options: Vec<PromptOption> = cards
                    .iter()
                    .enumerate()
                    .map(|(idx, c)| PromptOption {
                        index: idx,
                        option_id: idx.to_string(),
                        label: sanitize_ascii_typography(&format!(
                            "Pick {} (Cost {})",
                            c.title(),
                            c.cost()
                        )),
                        target_uid: Some(c.uid),
                        mode: Some("pick".to_string()),
                    })
                    .collect();
                self.captured_prompt = Some(ActivePrompt {
                    decision_id: format!("pick:{}", self.cursor),
                    kind: "pick".to_string(),
                    title: "Look at Revealed Deck Cards - Choose One".to_string(),
                    subtitle: "The chosen card enters your hand; others go to the bottom of your deck."
                        .to_string(),
                    options,
                    required_cards: 1,
                    candidate_cards: cards
                        .iter()
                        .map(|c| VisualCardData::from_engine_card(c, true))
                        .collect(),
                });
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
                    RecordedDecision::SelectedCards(uids) if uids.len() == count => {
                        let mut uniq = uids.clone();
                        uniq.sort_unstable();
                        uniq.dedup();
                        if uniq.len() == count
                            && uids.iter().all(|u| hand.iter().any(|c| c.uid == *u))
                        {
                            return Ok(uids);
                        }
                    }
                    RecordedDecision::OptionIndex(idx) => {
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
                let options: Vec<PromptOption> = hand
                    .iter()
                    .enumerate()
                    .map(|(idx, c)| PromptOption {
                        index: idx,
                        option_id: idx.to_string(),
                        label: sanitize_ascii_typography(&format!(
                            "Discard {} (Cost {})",
                            c.title(),
                            c.cost()
                        )),
                        target_uid: Some(c.uid),
                        mode: Some("discard".to_string()),
                    })
                    .collect();
                self.captured_prompt = Some(ActivePrompt {
                    decision_id: format!("discard:{}:{}", count, self.cursor),
                    kind: "discard".to_string(),
                    title: format!(
                        "Select {} Card{} to Discard",
                        count,
                        if count == 1 { "" } else { "s" }
                    ),
                    subtitle: "Click a card option below to confirm your discard selection."
                        .to_string(),
                    options,
                    required_cards: count,
                    candidate_cards: hand
                        .iter()
                        .map(|c| VisualCardData::from_engine_card(c, true))
                        .collect(),
                });
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

/// The headless Engine Bridge (`src/bridge.rs`).
/// Owns the authoritative `heav3ns_engine::engine::Game` state, processes `GameCommand`s from the
/// client, and translates engine log transitions into `VecDeque<VisualEvent>` for the `AnimationSequencer`.
pub struct GameBridge {
    pub game: Game,
    pub seed: u64,
    pub starter: usize,
    pub log_cursor: usize,
    pub preferred_target: Option<i32>,
    pub preferred_mode: Option<String>,
    pub card_specs: HashMap<String, Value>,
    /// When true, ambiguous modal choices, Counter windows, deck picks, and hand discards
    /// pause command execution and populate `VisualBoardState::prompt` for on-canvas player selection.
    pub interactive_prompts: bool,
    pub pending_command: Option<GameCommand>,
    pub recorded_decisions: Vec<RecordedDecision>,
    pub active_prompt: Option<ActivePrompt>,
}

impl GameBridge {
    pub fn new(seed: u64, starter: usize) -> Result<(Self, VecDeque<VisualEvent>, VisualBoardState), String> {
        Self::new_with_interactive(seed, starter, false)
    }

    pub fn new_with_interactive(
        seed: u64,
        starter: usize,
        interactive_prompts: bool,
    ) -> Result<(Self, VecDeque<VisualEvent>, VisualBoardState), String> {
        let specs = load_proposed_specs();
        let mut card_specs = HashMap::new();
        for s in specs {
            if let Some(id) = s.get("id").and_then(|v| v.as_str()) {
                card_specs.insert(id.to_string(), s);
            }
        }

        let mut game = stock_game(seed)?;
        game.trace_transitions = true;
        let mut bridge = Self {
            game,
            seed,
            starter,
            log_cursor: 0,
            preferred_target: None,
            preferred_mode: None,
            card_specs,
            interactive_prompts,
            pending_command: None,
            recorded_decisions: Vec::new(),
            active_prompt: None,
        };

        let mut provider = BridgeDecisionProvider::new(None, None, Vec::new(), interactive_prompts);
        bridge
            .game
            .start_with_providers(starter, &provider)
            .map_err(|e| e.0)?;

        // If AI starts, run AI turn and start Alice's turn (or pause if an interactive prompt arises)
        if starter == 1 {
            match bridge.run_ai_turn(&mut provider) {
                Ok(()) => {}
                Err(e) if e == "PROMPT_REQUIRED" => {
                    bridge.active_prompt = provider.captured_prompt;
                    bridge.pending_command = Some(GameCommand::EndTurn);
                }
                Err(e) => return Err(e),
            }
        }

        let events = bridge.drain_new_log_events();
        let board_state = bridge.build_visual_board_state();
        Ok((bridge, events, board_state))
    }

    /// Processes a discrete `GameCommand` from the client and emits a `VecDeque<VisualEvent>`
    /// alongside the authoritative post-command `VisualBoardState`.
    pub fn dispatch(
        &mut self,
        cmd: GameCommand,
    ) -> Result<(VecDeque<VisualEvent>, VisualBoardState), String> {
        let prev_interactive = self.interactive_prompts;
        match cmd {
            GameCommand::ResetMatch { seed, starter } => {
                let (new_bridge, events, state) =
                    Self::new_with_interactive(seed, starter, prev_interactive)?;
                *self = new_bridge;
                Ok((events, state))
            }
            GameCommand::TriggerProp { prop_index } => {
                let mut q = VecDeque::new();
                let prop_names = [
                    "Cheshire Lantern",
                    "Mad Hatter's Pocketwatch",
                    "Queen's Rose Bush",
                    "Drink-Me Vial",
                ];
                let name = prop_names.get(prop_index % 4).copied().unwrap_or("Corner Prop");
                q.push_back(VisualEvent::BannerMessage {
                    text: format!("{name} Activated"),
                    subtext: "Enchanted Table ambient prop triggered.".to_string(),
                });
                Ok((q, self.build_visual_board_state()))
            }
            GameCommand::SubmitDecision {
                option_index,
                selected_uids,
            } => {
                if let Some(ref prompt) = self.active_prompt {
                    let dec = if prompt.required_cards > 1 && selected_uids.len() >= prompt.required_cards {
                        RecordedDecision::SelectedCards(
                            selected_uids.into_iter().take(prompt.required_cards).collect(),
                        )
                    } else if let Some(idx) = option_index {
                        if prompt.kind == "discard" && prompt.required_cards == 1 {
                            if let Some(opt) = prompt.options.get(idx) {
                                if let Some(u) = opt.target_uid {
                                    RecordedDecision::SelectedCards(vec![u])
                                } else {
                                    RecordedDecision::OptionIndex(idx)
                                }
                            } else {
                                RecordedDecision::OptionIndex(idx)
                            }
                        } else {
                            RecordedDecision::OptionIndex(idx)
                        }
                    } else if !selected_uids.is_empty() {
                        RecordedDecision::SelectedCards(selected_uids)
                    } else {
                        RecordedDecision::OptionIndex(0)
                    };
                    self.recorded_decisions.push(dec);
                } else {
                    if let Some(&first_uid) = selected_uids.first() {
                        self.preferred_target = Some(first_uid);
                    }
                    if let Some(idx) = option_index {
                        let modes = [
                            "draw1",
                            "draw2",
                            "bounce_friendly",
                            "break1",
                            "recover_action",
                            "recover_any",
                        ];
                        if let Some(&m) = modes.get(idx) {
                            self.preferred_mode = Some(m.to_string());
                        }
                    }
                }
                self.active_prompt = None;
                if let Some(pending_cmd) = self.pending_command.take() {
                    return self.execute_with_prompt_support(pending_cmd);
                }
                let events = self.drain_new_log_events();
                Ok((events, self.build_visual_board_state()))
            }
            other_cmd => {
                self.active_prompt = None;
                self.pending_command = None;
                self.recorded_decisions.clear();
                self.execute_with_prompt_support(other_cmd)
            }
        }
    }

    fn execute_with_prompt_support(
        &mut self,
        cmd: GameCommand,
    ) -> Result<(VecDeque<VisualEvent>, VisualBoardState), String> {
        let mut provider = BridgeDecisionProvider::new(
            self.preferred_target,
            self.preferred_mode.clone(),
            self.recorded_decisions.clone(),
            self.interactive_prompts,
        );

        match cmd.clone() {
            GameCommand::PlayCard { card_id, target_id } => {
                if self.game.winner.is_some() {
                    return Err("Match has already ended.".to_string());
                }
                if self.game.active != Some(0) {
                    return Err("Wait for your turn.".to_string());
                }
                if let Some(t) = target_id {
                    self.preferred_target = Some(t);
                    provider.preferred_target = Some(t);
                }
                if self.game.phase == "resolved" {
                    self.game
                        .begin_sequence_with_providers(&provider)
                        .map_err(|e| e.0)?;
                }
                let mana_before = self.game.p[0].actions;
                self.game
                    .place_with_providers(card_id, &provider)
                    .map_err(|e| e.0)?;
                let mana_after = self.game.p[0].actions;

                let mut events = VecDeque::new();
                if mana_before > mana_after {
                    events.push_back(VisualEvent::ManaSpent {
                        seat: 0,
                        amount: mana_before - mana_after,
                        remaining: mana_after,
                    });
                }
                let mut log_events = self.drain_new_log_events();
                events.append(&mut log_events);
                self.recorded_decisions.clear();
                Ok((events, self.build_visual_board_state()))
            }
            GameCommand::Attack {
                attacker_id,
                defender_id,
            } => {
                if self.game.winner.is_some() {
                    return Err("Match has already ended.".to_string());
                }
                if self.game.active != Some(0) {
                    return Err("Wait for your turn.".to_string());
                }
                if defender_id >= 0 {
                    self.preferred_target = Some(defender_id);
                    provider.preferred_target = Some(defender_id);
                }
                let mut events = VecDeque::new();
                if self.game.phase == "placement" && !self.game.sequence.is_empty() {
                    let tr_len = self.game.transitions.len();
                    match self.game.close_with_providers(&mut provider) {
                        Ok(()) => {
                            events.append(&mut self.drain_new_log_events());
                            self.recorded_decisions.clear();
                        }
                        Err(e) if e.0 == "PROMPT_REQUIRED" => {
                            self.game.transitions.truncate(tr_len);
                            self.active_prompt = provider.captured_prompt;
                            self.pending_command = Some(cmd);
                            return Ok((events, self.build_visual_board_state()));
                        }
                        Err(e) => return Err(e.0),
                    }
                } else {
                    events.push_back(VisualEvent::DamageDealt {
                        attacker_uid: Some(attacker_id),
                        target_seat: 1,
                        target_uid: if defender_id >= 0 {
                            Some(defender_id)
                        } else {
                            None
                        },
                        amount: 1,
                        is_shield_break: false,
                    });
                }
                Ok((events, self.build_visual_board_state()))
            }
            GameCommand::ResolveSequence => {
                if self.game.winner.is_some() {
                    return Err("Match has already ended.".to_string());
                }
                if self.game.phase != "placement" {
                    return Err("No open sequence to resolve.".to_string());
                }
                let tr_len = self.game.transitions.len();
                match self.game.close_with_providers(&mut provider) {
                    Ok(()) => {
                        let events = self.drain_new_log_events();
                        self.recorded_decisions.clear();
                        Ok((events, self.build_visual_board_state()))
                    }
                    Err(e) if e.0 == "PROMPT_REQUIRED" => {
                        self.game.transitions.truncate(tr_len);
                        self.active_prompt = provider.captured_prompt;
                        self.pending_command = Some(cmd);
                        let events = self.drain_new_log_events();
                        Ok((events, self.build_visual_board_state()))
                    }
                    Err(e) => Err(e.0),
                }
            }
            GameCommand::BeginSequence => {
                if self.game.winner.is_some() {
                    return Err("Match has already ended.".to_string());
                }
                if self.game.phase != "resolved" {
                    return Err("Current sequence is already open.".to_string());
                }
                self.game
                    .begin_sequence_with_providers(&provider)
                    .map_err(|e| e.0)?;
                let events = self.drain_new_log_events();
                Ok((events, self.build_visual_board_state()))
            }
            GameCommand::EndTurn => {
                if self.game.winner.is_some() {
                    return Err("Match has already ended.".to_string());
                }
                if self.game.active == Some(0) {
                    if self.game.phase == "placement" {
                        let tr_len = self.game.transitions.len();
                        match self.game.close_with_providers(&mut provider) {
                            Ok(()) => {
                                self.recorded_decisions.clear();
                                provider.recorded.clear();
                                provider.cursor = 0;
                            }
                            Err(e) if e.0 == "PROMPT_REQUIRED" => {
                                self.game.transitions.truncate(tr_len);
                                self.active_prompt = provider.captured_prompt;
                                self.pending_command = Some(cmd);
                                let events = self.drain_new_log_events();
                                return Ok((events, self.build_visual_board_state()));
                            }
                            Err(e) => return Err(e.0),
                        }
                    }
                    if self.game.winner.is_none() && self.game.phase != "ended" {
                        let excess = self.game.p[0].hand.len().saturating_sub(6);
                        let discards = if excess > 0 {
                            match provider.discard(&self.game, 0, excess) {
                                Ok(d) => d,
                                Err(e) if e.0 == "PROMPT_REQUIRED" => {
                                    self.active_prompt = provider.captured_prompt;
                                    self.pending_command = Some(GameCommand::EndTurn);
                                    let events = self.drain_new_log_events();
                                    return Ok((events, self.build_visual_board_state()));
                                }
                                Err(e) => return Err(e.0),
                            }
                        } else {
                            Vec::new()
                        };
                        self.game
                            .end_turn_with_providers(&discards, &provider)
                            .map_err(|e| e.0)?;
                        self.recorded_decisions.clear();
                        provider.recorded.clear();
                        provider.cursor = 0;
                    }
                } else if self.game.active != Some(1) {
                    return Err("Wait for your turn.".to_string());
                }

                // Run or resume White Rabbit AI turn automatically if game continues
                if self.game.winner.is_none() {
                    match self.run_ai_turn(&mut provider) {
                        Ok(()) => {}
                        Err(e) if e == "PROMPT_REQUIRED" => {
                            self.active_prompt = provider.captured_prompt;
                            self.pending_command = Some(GameCommand::EndTurn);
                            let events = self.drain_new_log_events();
                            return Ok((events, self.build_visual_board_state()));
                        }
                        Err(e) => return Err(e),
                    }
                }
                self.preferred_target = None;
                self.preferred_mode = None;
                self.recorded_decisions.clear();
                let events = self.drain_new_log_events();
                Ok((events, self.build_visual_board_state()))
            }
            _ => unreachable!(),
        }
    }

    fn run_ai_turn(&mut self, provider: &mut BridgeDecisionProvider) -> Result<(), String> {
        if self.game.active != Some(1) {
            self.game
                .start_with_providers(1, provider)
                .map_err(|e| e.0)?;
        }
        while self.game.winner.is_none() {
            // If a sequence is already placed on the field (e.g. resuming after PROMPT_REQUIRED), close it first
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
                        return Err(e.0);
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
                        return Err(e.0);
                    }
                    self.recorded_decisions.clear();
                    provider.recorded.clear();
                    provider.cursor = 0;
                }
                break;
            }
            if self.game.phase == "resolved" {
                self.game
                    .begin_sequence_with_providers(provider)
                    .map_err(|e| e.0)?;
            }
            for c in chosen {
                self.game
                    .place_with_providers(c.uid, provider)
                    .map_err(|e| e.0)?;
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
                    return Err(e.0);
                }
            }
        }
        if self.game.phase != "ended" {
            let excess = self.game.p[1].hand.len().saturating_sub(6);
            let discard = if self.game.winner.is_some() || excess == 0 {
                Vec::new()
            } else {
                provider
                    .ai
                    .discard(&self.game, 1, excess)
                    .map_err(|e| e.0)?
            };
            self.game
                .end_turn_with_providers(&discard, provider)
                .map_err(|e| e.0)?;
        }
        if self.game.winner.is_none() && self.game.active != Some(0) {
            self.game
                .start_with_providers(0, provider)
                .map_err(|e| e.0)?;
        }
        Ok(())
    }

    /// Converts new entries in `self.game.log` since `self.log_cursor` into ordered `VisualEvent`s.
    pub fn drain_new_log_events(&mut self) -> VecDeque<VisualEvent> {
        let mut out = VecDeque::new();
        while self.log_cursor < self.game.log.len() {
            let ev = &self.game.log[self.log_cursor];
            self.log_cursor += 1;
            let kind = ev.first().and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "start" => {
                    let seat = ev.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    out.push_back(VisualEvent::TurnStarted {
                        seat,
                        turn_number: self.game.turn_number.max(1) as usize,
                    });
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
                    let (spec_id, title) = uid
                        .and_then(|u| self.game.cards.get(&u))
                        .map(|c| (c.id().to_string(), sanitize_ascii_typography(c.title())))
                        .unwrap_or_else(|| ("H3W-AL-001".to_string(), "Card".to_string()));
                    out.push_back(VisualEvent::CardDrawn {
                        seat,
                        card_uid: if seat == 0 { uid } else { None },
                        card_spec_id: spec_id,
                        title,
                    });
                }
                "place" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            if c.owner == 1 {
                                out.push_back(VisualEvent::ManaSpent {
                                    seat: 1,
                                    amount: c.cost(),
                                    remaining: self.game.p[1].actions,
                                });
                            }
                            if c.card_type() == "Character" {
                                let slot_index = self.game.p[c.owner]
                                    .field
                                    .iter()
                                    .position(|fc| fc.uid == uid)
                                    .unwrap_or(0);
                                out.push_back(VisualEvent::UnitSummoned {
                                    seat: c.owner,
                                    card_uid: uid,
                                    card_spec_id: c.id().to_string(),
                                    title: sanitize_ascii_typography(c.title()),
                                    cost: c.cost(),
                                    card_type: c.card_type().to_string(),
                                    slot_index,
                                });
                            } else {
                                out.push_back(VisualEvent::SpellCast {
                                    seat: c.owner,
                                    card_uid: uid,
                                    card_spec_id: c.id().to_string(),
                                    title: sanitize_ascii_typography(c.title()),
                                    target_uid: self.preferred_target,
                                });
                            }
                        }
                    }
                }
                "counter" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        let target_uid = ev.get(2).and_then(|v| v.as_i64()).map(|u| u as i32);
                        if let Some(c) = self.game.cards.get(&uid) {
                            out.push_back(VisualEvent::SpellCast {
                                seat: c.owner,
                                card_uid: uid,
                                card_spec_id: c.id().to_string(),
                                title: sanitize_ascii_typography(c.title()),
                                target_uid,
                            });
                        }
                    }
                }
                "reveal" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            out.push_back(VisualEvent::DamageDealt {
                                attacker_uid: None,
                                target_seat: c.owner,
                                target_uid: Some(uid),
                                amount: 1,
                                is_shield_break: true,
                            });
                            out.push_back(VisualEvent::ShieldRevealed {
                                seat: c.owner,
                                card_uid: uid,
                                card_spec_id: c.id().to_string(),
                                title: sanitize_ascii_typography(c.title()),
                                trigger_text: c.shield_trigger().map(sanitize_ascii_typography),
                            });
                        }
                    }
                }
                "return" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            out.push_back(VisualEvent::UnitBounced {
                                seat: c.owner,
                                card_uid: uid,
                                title: sanitize_ascii_typography(c.title()),
                            });
                        }
                    }
                }
                "recover" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            out.push_back(VisualEvent::CardRecovered {
                                seat: c.owner,
                                card_uid: uid,
                                title: sanitize_ascii_typography(c.title()),
                            });
                        }
                    }
                }
                "sequence_discard" | "discard" | "end_discard" | "shield_discard" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            let reason = match kind {
                                "sequence_discard" => "Sequence resolved",
                                "discard" => "Discarded by effect",
                                "end_discard" => "Hand limit discard",
                                _ => "Shield broken",
                            };
                            out.push_back(VisualEvent::UnitDestroyed {
                                seat: c.owner,
                                card_uid: uid,
                                title: sanitize_ascii_typography(c.title()),
                                reason: reason.to_string(),
                            });
                        }
                    }
                }
                "cancel" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        let k_str = ev.get(2).and_then(|v| v.as_str()).unwrap_or("effect");
                        if let Some(c) = self.game.cards.get(&uid) {
                            out.push_back(VisualEvent::EffectCancelled {
                                card_uid: uid,
                                title: sanitize_ascii_typography(c.title()),
                                kind: k_str.to_string(),
                            });
                        }
                    }
                }
                "combo_lost_support" => {
                    if let Some(uid) = ev.get(1).and_then(|v| v.as_i64()).map(|u| u as i32) {
                        if let Some(c) = self.game.cards.get(&uid) {
                            out.push_back(VisualEvent::ComboLostSupport {
                                card_uid: uid,
                                title: sanitize_ascii_typography(c.title()),
                            });
                        }
                    }
                }
                "win" => {
                    let seat = ev.get(1).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    out.push_back(VisualEvent::MatchWon { winner_seat: seat });
                }
                _ => {}
            }
        }
        out
    }

    pub fn build_visual_board_state(&self) -> VisualBoardState {
        let g = &self.game;
        let can_play_cards = g.winner.is_none()
            && g.active == Some(0)
            && (g.phase == "placement" || g.phase == "resolved");

        let player_hand: Vec<VisualCardData> = g.p[0]
            .hand
            .iter()
            .map(|c| {
                let playable =
                    can_play_cards && c.card_type() != "Counter" && c.cost() <= g.p[0].actions;
                VisualCardData::from_engine_card(c, playable)
            })
            .collect();

        let board_0: Vec<VisualCardData> = g.p[0]
            .field
            .iter()
            .map(|c| VisualCardData::from_engine_card(c, false))
            .collect();
        let board_1: Vec<VisualCardData> = g.p[1]
            .field
            .iter()
            .map(|c| VisualCardData::from_engine_card(c, false))
            .collect();

        let active_sequence: Vec<VisualCardData> = g
            .sequence
            .iter()
            .filter(|c| g.present(c.uid))
            .map(|c| VisualCardData::from_engine_card(c, false))
            .collect();

        let status_line = if let Some(ref p) = self.active_prompt {
            format!("DECISION REQUIRED: {} - {}", p.title, p.subtitle)
        } else if let Some(w) = g.winner {
            format!(
                "{} Wins the Match! Press [R] or click New Match to play again.",
                if w == 0 { "Alice" } else { "White Rabbit" }
            )
        } else if g.active == Some(0) {
            if g.phase == "placement" {
                if g.sequence.is_empty() {
                    "Alice's Turn - Drag a glowing card to the table, or click End Turn.".to_string()
                } else {
                    format!(
                        "Sequence Open ({} card{}) - Drag more cards to combo, or click Resolve Sequence.",
                        g.sequence.len(),
                        if g.sequence.len() == 1 { "" } else { "s" }
                    )
                }
            } else {
                "Sequence Resolved - Drag another card to start a new sequence, or click End Turn."
                    .to_string()
            }
        } else {
            "White Rabbit is thinking...".to_string()
        };

        let mut combat_log = Vec::new();
        for ev in g.log.iter().rev().take(12).rev() {
            let k = ev.first().and_then(|v| v.as_str()).unwrap_or("");
            match k {
                "start" => {
                    let s = ev.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
                    combat_log.push(format!(
                        "{} begins a turn.",
                        if s == 0 { "Alice" } else { "White Rabbit" }
                    ));
                }
                "place" => {
                    if let Some(u) = ev.get(1).and_then(|v| v.as_i64()) {
                        if let Some(c) = g.cards.get(&(u as i32)) {
                            combat_log.push(sanitize_ascii_typography(&format!("Played {}.", c.title())));
                        }
                    }
                }
                "reveal" => {
                    if let Some(u) = ev.get(1).and_then(|v| v.as_i64()) {
                        if let Some(c) = g.cards.get(&(u as i32)) {
                            combat_log.push(sanitize_ascii_typography(&format!("Shield revealed: {}.", c.title())));
                        }
                    }
                }
                "return" => {
                    if let Some(u) = ev.get(1).and_then(|v| v.as_i64()) {
                        if let Some(c) = g.cards.get(&(u as i32)) {
                            combat_log.push(sanitize_ascii_typography(&format!("Returned {} to hand.", c.title())));
                        }
                    }
                }
                "cancel" => {
                    if let Some(u) = ev.get(1).and_then(|v| v.as_i64()) {
                        if let Some(c) = g.cards.get(&(u as i32)) {
                            combat_log.push(sanitize_ascii_typography(&format!("Cancelled {}.", c.title())));
                        }
                    }
                }
                "win" => {
                    let s = ev.get(1).and_then(|v| v.as_i64()).unwrap_or(0);
                    combat_log.push(format!(
                        "{} wins!",
                        if s == 0 { "Alice" } else { "White Rabbit" }
                    ));
                }
                _ => {}
            }
        }

        VisualBoardState {
            seed: self.seed,
            turn_number: g.turn_number.max(1) as usize,
            active_seat: g.active.unwrap_or(0),
            phase: g.phase.clone(),
            mana: [g.p[0].actions, g.p[1].actions],
            max_mana: [6, 6],
            shields_count: [g.p[0].shields.len(), g.p[1].shields.len()],
            deck_count: [g.p[0].deck.len(), g.p[1].deck.len()],
            discard_count: [g.p[0].discard.len(), g.p[1].discard.len()],
            opponent_hand_count: g.p[1].hand.len(),
            player_hand,
            board_units: [board_0, board_1],
            active_sequence,
            winner: g.winner,
            status_line,
            prompt: self.active_prompt.clone(),
            combat_log,
        }
    }

    pub fn export_metadata_json(&self) -> Value {
        json!({
            "seed": self.seed,
            "turn": self.game.turn_number,
            "phase": self.game.phase,
            "cards_loaded": self.card_specs.len(),
        })
    }
}
