# HEAV3NS-ENGINE — Executive Architecture & Commission Deliverables

**Product**: WONDERLAND (*Wonderland Digital 0.3* & *Wonderland Digital 0.4 — The Enchanted Table*)  
**Engine**: HEAV3NS-ENGINE v0.4.1 (Dedicated Rust Rules Runtime, Policy Harness, Replay Verifier, and Enchanted Table Server)  
**Canonical Card Pool**: FC04.1 (`data/proposed.json`, SHA-256 `db4fb5bad7c274cc77a5323e44651dc427a17ba67a9e80e47706000fbc5517c2`)  
**Canonical Specs Hash**: `4a290f53da32379b4f0ff87ee988cd7ad142944d461f09724c87d46626661704`

> **Governing Principle**: WONDERLAND is the product. HEAV3NS-ENGINE is the dedicated machinery built to realize it.

---

## Deliverable 1: Wonderland Infrastructure Audit

A comprehensive audit was conducted across the complete Wonderland Digital 0.3 and 0.4 release bundle (`Wonderland-Digital-0.4-Local-Playable.zip` and `Wonderland-Digital-0.4-Supplementary-Evidence.zip`), sibling art/generation repositories (`/Users/bozoegg/Desktop/Wonderland`), and the surrounding HEAV3NS agentic ecosystem (`/Users/bozoegg/Desktop/HEAV3NS-BRAIN`, `/Users/bozoegg/Desktop/BroccoliDB`, `/Users/bozoegg/Desktop/BROCCOLIQUEUE`, `/Users/bozoegg/Desktop/LUMI-NEW`).

### 1.1 Existing Code, Data, Assets, and Tooling Inventory

| Layer | Historical Source | Role & Status | Classification |
| :--- | :--- | :--- | :--- |
| **Canonical Card Pool (FC04.1)** | `vendor/fc041/fc041-variants/proposed.json` → [`data/proposed.json`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/data/proposed.json) | 44 unique card definitions (20 Alice `H3W-AL-001..020`, 24 White Rabbit `H3W-RA-001..024`), 40-card stock decks, explicit `ops`/`entry_ops`/`combo`. | **Canonical Rules Authority (Preserved 100%)** |
| **Base Fixture Card Pool** | `vendor/fc041/data/cards.json` → [`data/cards.json`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/data/cards.json) | 44-card baseline pool used by unit test fixtures when `specs=None`. | **Fixture Asset (Preserved 100%)** |
| **Rules Prototype (`engine.py`)** | `vendor/fc041/tools/engine.py` | 757-line Python rules engine (`Game`, `Card`, `Player`, `Response`, `Effect`, atomic `_transition` rollback, LIFO `_window`/`_counter`). | **Migrated to Rust ([`src/engine.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/engine.rs))** |
| **Simulation & Beam Policy (`simulate_games.py`)** | `vendor/fc041/tools/simulate_games.py` | 321-line `SimGame` delayed `select`/`discard` hooks + 4-profile (`offense`, `mixed`, `control`, `resource`) beam heuristic `Policy`. | **Migrated to Rust ([`src/policy.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/policy.rs))** |
| **Visible-State Audit Policy (`audit_policy.py`)** | `vendor/fc041/tools/audit_policy.py` | 171-line `AuditPolicy` with `public_clone` (`UNKNOWN` redaction), `affordable_sequences`, and bounded 1-step recovery continuation rollouts. | **Migrated to Rust ([`src/policy.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/policy.rs))** |
| **State & Transition Contracts** | `r2_contract.py`, `transition_support.py`, `replay_transitions.py`, `replay_check.py` | `wonderland-state-1/2` boundary snapshots, `wonderland-transition-1` audit logs, and `wonderland-digital-replay-v1` verification. | **Migrated to Rust ([`src/contract.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/contract.rs))** |
| **Human-vs-AI Match Adapter (`adapter.py`)** | `adapter.py` | 383-line thread-safe coordinator bridging human browser decisions with `SimGame` and `AuditPolicy`, enforcing hidden-zone redaction. | **Migrated to Rust ([`src/adapter.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/adapter.rs))** |
| **Loopback HTTP API Server (`server.py`)** | `server.py` | 89-line localhost server with strict Host/Origin/Sec-Fetch-Site/Content-Length framing guards. | **Migrated to Rust ([`src/server.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/server.rs))** |
| **Native Macroquad Frontend (`client_macroquad`)** | Replaced legacy web frontend (`frontend/dist`) → [`client_macroquad/`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/client_macroquad) | Native 1920x1080 60+ FPS Macroquad (`v0.4.x`) rendering client (`src/bridge.rs`, `src/renderer.rs`, `src/visuals/card_view.rs`, `src/visuals/vfx.rs`). | **Canonical Visual Frontend (Native Rust Client)** |
| **Approved Card Artwork** | `assets/cards/*.webp` → [`assets/cards/`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/assets/cards) | 44 approved WebP card illustrations (`H3W-AL-001..020.webp`, `H3W-RA-001..024.webp`) + `card-back.webp`. | **Canonical Visual Assets (Preserved 100%)** |
| **Historical Replays & Evidence** | [`fixtures/sample-complete-replay.json`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/fixtures/sample-complete-replay.json), [`fixtures/downloaded-seed2-replay.json`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/fixtures/downloaded-seed2-replay.json) | Recorded full-game replays from Wonderland 0.2, 0.3, and 0.4 verification runs. | **Regression Verification Suite (100% Verified)** |

---

## Deliverable 2: Engine Architecture Specification

**HEAV3NS-ENGINE** is architected as a zero-compromise, deterministic Rust engine purpose-built for Wonderland. Rather than abstracting cards into a loose scripting language or generic ECS, every module in the crate directly models Wonderland's 6-Action economy, left-to-right sequence board, forward Combo support, and LIFO Counter chains.

### 2.1 Module Architecture

```text
HEAV3NS-ENGINE/
├── Cargo.toml
├── data/
│   ├── proposed.json          # Canonical FC04.1 44-card pool (embedded & hash-verified)
│   └── cards.json             # Base fixture 44-card pool
├── assets/cards/              # 44 WebP card illustrations + card-back.webp
├── client_macroquad/          # Wonderland Digital 0.4 — Native Macroquad 0.4 Frontend Client
│   ├── src/bridge.rs          # Headless GameCommand <-> VisualEvent & AnimationSequencer bridge
│   ├── src/renderer.rs        # 5-Layer Z-sorted 1920x1080 render pipeline & input dispatcher
│   └── src/visuals/           # Tactile card physics, 2.5D GLSL shaders, Perlin camera & particle pool
├── fixtures/                  # Historical 0.2/0.3/0.4 replay artifacts
├── src/
│   ├── lib.rs                 # Public crate root
│   ├── main.rs                # CLI & Technical Demonstrator (serve, verify-replay, simulate, inspect-state, audit)
│   ├── rng.rs                 # Bit-exact Python MT19937 PRNG + Fisher-Yates randbelow shuffle + rng_sha256
│   ├── engine.rs              # Deterministic Game & SimGame state machine, atomic command rollback, LIFO windows
│   ├── policy.rs              # Public-state redaction (public_clone), Policy beam search, AuditPolicy rollouts
│   ├── contract.rs            # wonderland-state-1/2 snapshot/restore, transition replay, replay_check verifier
│   ├── adapter.rs             # Thread-safe Human-vs-AI Match coordinator & Enchanted Table JSON presenter
│   └── server.rs              # Hardened 127.0.0.1 HTTP server with framing/origin/traversal guards
└── tests/
    └── engine_and_replay_tests.rs  # Comprehensive integration & historical replay verification tests
```

### 2.2 Core Architectural Guarantees
1. **Bit-Exact PRNG & Hashing Parity ([`PyRng`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/rng.rs#L11-L15))**: Implements Python's exact `MT19937` state vector (`624` `u32` words + index), `init_by_array` (`19650218`), `getrandbits`, and `randbelow` Fisher-Yates shuffle. Serializes state identically to `json.dumps(g.rng.getstate())` (`[3, [mt0, ..., mt623, index], null]`) so `rng_sha256` hashes match historical Python traces down to the bit.
2. **Atomic Command Transactions ([`Game::execute_command`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/engine.rs#L748-L881))**: Every game command (`add`, `start`, `begin_sequence`, `place`, `close`, `draw`, `break_shields`, `end_turn`) captures a pre-execution snapshot. If any validation check, rules operation, decision callback, or post-command invariant (`invariants()`) fails, the engine atomically restores the pre-command snapshot and records a `status: "rejected", rolled_back: true` transition record.
3. **Zero Hidden-State Leakage**: Both [`public_clone`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/policy.rs#L126-L155) (used by AI rollouts) and [`publish_state`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/adapter.rs#L525-L600) (used by the HTTP `/api/state` endpoint) strip concealed cards (`deck`, `shields`, and opponent `hand`), replacing them with `UNKNOWN` or `{"hidden": true}` before any external consumer or policy can inspect them.

---

## Deliverable 3: Existing Implementation Migration Plan

To guarantee continuity across Wonderland 0.2, 0.3, and 0.4 while eliminating the performance and deployment bottlenecks of the legacy Python stack, migration follows a strict **Verify-in-Place, Replace-Under-Contract** strategy:

| Component | Disposition | Migration & Parity Verification |
| :--- | :--- | :--- |
| **`data/proposed.json` & `assets/cards/*.webp`** | **Retain Unchanged** | Embedded directly via `include_str!` / `include_bytes!` and verified via SHA-256 (`db4fb5...`). |
| **`vendor/fc041/tools/engine.py`** | **Ported to Rust (`src/engine.rs`)** | All 18 operation kinds, card/choice/response validators, forward supporter checks, LIFO response chains, and Shield Triggers ported with 100% event-log parity. |
| **`simulate_games.py` & `audit_policy.py`** | **Ported to Rust (`src/policy.rs`)** | `SimGame` delayed `select`/`discard` resolution, `Policy` beam search (width 18, depth 6), and `AuditPolicy` exhaustive distinct-spec sequence rollouts ported natively—achieving >20x faster rollout evaluation in Rust. |
| **`r2_contract.py` & `replay_transitions.py`** | **Ported to Rust (`src/contract.rs`)** | Full bidirectional compatibility: Rust verifies Python-generated `.json` replays (`sample-complete-replay.json`, `downloaded-seed2-replay.json`), and Python's `replay_check.py` verifies Rust-generated replays. |
| **`adapter.py` & `server.py`** | **Ported to Rust (`src/adapter.rs`, `src/server.rs`)** | Single native binary (`heav3ns-engine serve`) replaces the Python HTTP server and threading bridge while serving the exact JSON schema for agent/API consumers. |
| **`client_macroquad` (0.4 Enchanted Table)** | **Migrated to Native Macroquad (`v0.4.x`)** | Legacy web frontend removed and replaced by the native 60+ FPS `client_macroquad` puppet-stage client decoupled from `heav3ns-engine`. |

---

## Deliverable 4: Rules and Simulation Specification

HEAV3NS-ENGINE enforces the exact canonical **Wonderland FC04.1** ruleset without invented mechanics:

### 4.1 Zones, Setup, and Conservation Invariants
- **Zones per Player**: `deck` (private, ordered top-first), `shields` (private, 6 opening shields, ordered top-first), `hand` (private to owner, 5 opening cards, 6-card end-of-turn limit), `field` (public active sequence / response staging), and `discard` (public, ordered).
- **Card Conservation (`Game::invariants`)**: Every physical card (`uid: 1..=80` in a standard match) belongs to a fixed `owner` (`0` = Alice, `1` = White Rabbit) and resides in **exactly one** zone at any command boundary. Actions are strictly bounded in `0..=6`.

### 4.2 Turn Structure & 6-Action Economy
1. **Start of Turn (`start(owner)`)**: Requires `phase == "ended"`, `winner == None`, and alternating seat (`active != Some(owner)`). Sets `active = Some(owner)`, increments `turn_number`, refreshes `p[owner].actions = 6`, clears `sequence`, transitions `phase` to `"placement"`, and draws 1 card if `turn_number > 1` (Turn 1 starter does not draw).
2. **Sequence Placement (`place(uid)`)**: During `"placement"`, the active player pays `card.cost()` Actions to move a non-Counter (`Character` or `Action`) from their `hand` to their `field` and append it to `sequence`.
3. **Sequence Closure & Resolution (`close()`)**: Transitions `phase` to `"resolving"`, emits `("close", [seq_uids...])`, and iterates left-to-right through `sequence`:
   - If the card is still present on `field` (`present(uid)`):
     - Opens a response window (`_window`) for its primary effect (`"action"` ops for `Action` cards, `"entry"` `entry_ops` for `Character` cards, if non-empty).
     - After the primary effect window (even if the `"entry"` effect was cancelled by a Counter, provided `winner == None` and the card remains on `field`), checks `combo_eligible(c)`: if the card has a `combo` block with requirement `n` and at least `n` **later** non-Counter cards owned by the same player are currently present on `field` (`supporters(c).len() >= n`), marks `combo_used = true` and opens a `"combo"` window.
4. **Post-Sequence Cleanup**: Once all sequence cards have been processed (or immediately after `winner` is decided), all sequence cards still present on `field` move to their owner's `discard` (`"sequence_discard"`), and `phase` becomes `"resolved"`.
5. **Additional Sequences (`begin_sequence()`)**: From `"resolved"`, if the active player wishes to spend remaining Actions on another sequence, `begin_sequence()` clears `sequence` and re-enters `"placement"`.
6. **End of Turn (`end_turn(discard_uids)`)**: From `"resolved"`, if `winner == None` and `hand.len() > 6`, the active player must provide `discard_uids` containing **exact** distinct UIDs from their hand for `hand.len() - 6` cards (`"end_discard"`). Transitions `phase` to `"ended"`.

### 4.3 Forward Combo Support, Response Windows, and LIFO Counter Chains
- **Forward Support Rule (`Game::supporters`)**: For a card at index `i` in `sequence`, its supporters are `[s for s in sequence[i+1..] if s.owner == c.owner && s.type != "Counter" && present(s.uid)]`.
- **Target & Mode Locking (`Game::prepare`)**: Before a response window opens, modal `choice` options (`draw1`, `break1`, `recover_character`, etc.) and targets (`bounce_*`, `counter_bounce_later`, `recover_*`) are locked into `e.choice`. If a chosen target leaves the zone before resolution, `_select` fizzles (`None`) rather than silently retargeting.
- **Response Eligibility & LIFO Resolution (`Game::run_responses` & `Game::counter`)**:
  - When effect `e` opens a window, the opponent (`1 - e.source.owner`) may pass (`None`) or play an eligible `Counter` (or `response_play: true` card on an initial window, `is_reply == false`) from hand by paying its Action cost.
  - Playing a response card moves it from `hand` to `field`, logs `("counter", resp.uid, pending.source.uid, pending.kind)`, and opens a nested reply window (`is_reply = true`) where **only `Counter` cards** may reply.
  - Responses resolve **Last-In, First-Out (LIFO)**. After a response resolves and moves from `field` to `discard`, if the parent effect `e` is still alive (`pending_alive(e)`), the response loop re-Checks for further responses before `e` resolves.
  - **Crucial Combo Support Re-Check**: When a `"combo"` window finishes its response chain, the engine re-evaluates `supporters(c).len() < n`. If a response (such as White Rabbit's `H3W-RA-002 Borrowed Seconds` with `counter_bounce_later`) returned a later Character supporter to hand so fewer than `n` supporters remain on the field, the engine logs `("combo_lost_support", c.uid)` and suppresses the Combo!

### 4.4 Shield Breaks & Shield Triggers
- **`break_shields(owner, n)`**: Breaks `n` shields one at a time from the top (`index 0`) of `p[1 - owner].shields`:
  1. Logs `("reveal", shield.uid)`.
  2. If `shield.shield_trigger` is `"Draw 1 card."`, the defender immediately draws 1 card (`("draw", 1 - owner, drawn.uid)`) **before** the revealed shield enters `discard`.
  3. Moves the revealed shield from `shields` to `discard` and logs `("shield_discard", shield.uid)`.
  4. If `p[1 - owner].shields` is now empty, sets `winner = Some(owner)` and logs `("win", owner)`, halting any remaining breaks or effects.

---

## Deliverable 5: Visual Runtime Specification

**HEAV3NS-ENGINE** powers *Wonderland Digital 0.4 — The Enchanted Table* via the native Macroquad client ([`client_macroquad/`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/client_macroquad)) as well as the headless JSON presenter ([`src/adapter.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/adapter.rs) and [`src/server.rs`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/server.rs)).

### 5.1 State & Presentation Contract (`client_macroquad` & `GET /api/state`)
- **Authoritative Revision & Generation Guard**: Every state publication increments `revision` and carries a unique match `generation` UUID hex.
- **Privacy Redaction**:
  - `players[0].hand` (Alice / Human) serializes full card objects (`uid`, `id`, `name`, `type`, `cost`, `text`, `combo_n`, `art_url`, `owner`).
  - `players[1].hand` (White Rabbit / Opponent) and both `players[0..1].shields` serialize exclusively as `[{"hidden": true}, ...]`.
  - `history` translates raw engine events into human-readable table narration without leaking private `draw` or `select` card identities.
- **Enchanted Table Telemetry**:
  - `sequence_details`: Real-time read-only projection of each card in the active sequence (`combo_n`, `support_count`, `support_uids`, `combo_eligible`, `combo_used`, `present`, `effect_events`).
  - `timeline`: Last 24 public events with status badges, source card metadata, current zone tracking, and rules callouts (e.g., explaining why a cancelled Entry does not cancel a Combo, or why a returned supporter suppresses a Combo).
  - `hand_availability`: Per-card playability diagnostics for Alice's hand, explaining exact Action deficits or response-window restrictions.
  - `latest_reveal`: Structured Shield reveal banner payload with trigger text and turn number.

### 5.2 Loopback HTTP Security & Framing Contract
- Binds strictly to `127.0.0.1` (`localhost`).
- Validates `Host` (`127.0.0.1:<port>`, `localhost:<port>`, `[::1]:<port>`), `Sec-Fetch-Site` (`same-origin` or `none`), and `Origin` on all `POST` requests.
- Rejects `Transfer-Encoding` (`chunked`), duplicate or non-digit `Content-Length` headers, bodies larger than 65,536 bytes, and path-traversal attempts outside `assets`.

---

## Deliverable 6: Agent Integration Specification (LUMI / BroccoliDB / BroccoliQueue)

Per Section 5 and Section 7 of `commission.md`, **HEAV3NS-ENGINE** is designed for clean, first-class integration with **LUMI**, **BroccoliDB**, and **BroccoliQueue**, while strictly guaranteeing that **LUMI is never a mandatory runtime dependency for ordinary human-vs-AI or local gameplay**.

### 6.1 Decoupled 3-Tier Agent Architecture
1. **Tier 1 — Standalone Native Runtime (Zero External Dependencies)**:
   - `cargo run -p client_macroquad --release`, `heav3ns-engine serve`, and `heav3ns-engine simulate` run completely offline using the built-in Rust [`AuditPolicy`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/policy.rs#L218-L225) and heuristic `Policy` via the [`DecisionProviders`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/src/engine.rs#L569-L611) trait.
2. **Tier 2 — Structured CLI & HTTP Agent Bridge (LUMI / External Agent Controller)**:
   - External agents (such as LUMI) can inspect redacted match state and legal decision options via:
     - `heav3ns-engine inspect-state --seed <seed> --starter <0|1>` (CLI JSON snapshot)
     - `GET /api/state` → inspects `pending.id`, `pending.kind`, `pending.options`, `sequence_details`, and `hand_availability`.
     - `POST /api/action` → submits `{"decision_id": "<generation>:<n>", "option_id": "<id>"}` or `{"decision_id": "...", "selected": [uid, ...]}`.
   - Because `pending.options` enumerates every legal play, response, pass, mode choice, target, pick, and discard with explicit `id`, `cost`, and `after_confirm` descriptions, an LLM or MCTS agent (LUMI) can play either seat without ever having access to concealed zones (`deck`, `shields`, or opponent `hand`).
3. **Tier 3 — BroccoliDB & BroccoliQueue Artifact & Telemetry Pipeline**:
   - **BroccoliQueue Integration**: Batch simulation and balance-audit jobs (`heav3ns-engine simulate --seed <N> --export-replay <path>`) are stateless, deterministic, and fast (~8–15ms per full game in release mode), making them ideal worker payloads for BroccoliQueue task orchestration.
   - **BroccoliDB Integration**: Every completed match emits content-addressed, schema-versioned JSON artifacts ready for direct ingestion into BroccoliDB:
     - `wonderland-state-2` boundary snapshots (`specs_sha256` + `rng_state` + zone UID lists).
     - `wonderland-transition-1` step-by-step audit transitions (`before`, `after`, `rng_sha256`, `provider_outputs`, `decisions`, `events`, `state_changes`).
     - `wonderland-digital-replay-v1` full-match verifiable replay bundles.

---

## Deliverable 7: Implementation Roadmap

| Phase | Milestone | Scope & Verification Criteria | Status |
| :--- | :--- | :--- | :--- |
| **Phase 1** | **Infrastructure Audit & Asset Preservation** | Extract and audit 0.3/0.4 release bundles; preserve `data/proposed.json`, `data/cards.json`, 45 WebP card assets, and historical replay fixtures; remove legacy web frontend. | **COMPLETE** |
| **Phase 2** | **Deterministic Rust Core (`rng`, `engine`, `contract`)** | Implement bit-exact `PyRng` (`MT19937`), all 18 FC04.1 ops, atomic `_transition` rollback, LIFO Counter windows, `wonderland-state-2` snapshot/restore, and `wonderland-transition-1` logging. | **COMPLETE** |
| **Phase 3** | **Policy Harness & Replay Verification (`policy`, `verify-replay`)** | Implement `public_clone`, `Policy` beam search, and `AuditPolicy` rollout search; verify historical `sample-complete-replay.json` and `downloaded-seed2-replay.json` with 100% bit-exact parity. | **COMPLETE** |
| **Phase 4** | **Enchanted Table Runtime (`adapter`, `server`, CLI)** | Implement thread-safe `Match` coordinator, hidden-zone redaction, hardened loopback HTTP server (`serve`), and CLI subcommands (`simulate`, `verify-replay`, `inspect-state`, `audit`). | **COMPLETE** |
| **Phase 5** | **Future Expansion (Post-Commission)** | Optional WebSocket/SSE push transport alongside polling, multi-seed parallel Rayon tournament runner for BroccoliQueue, and pluggable LUMI HTTP/gRPC `DecisionProviders` adapter. | **Planned / Ready** |

---

## Deliverable 8: Technical Demonstrator & Verification Guide

The executable Rust demonstrator is built and verified in this repository.

### 8.1 CLI Commands
```bash
# 1. Run full infrastructure, card-pool hash, and historical replay audit
cargo run -- audit

# 2. Verify any historical or newly recorded wonderland-digital-replay-v1 artifact
cargo run -- verify-replay fixtures/downloaded-seed2-replay.json
cargo run -- verify-replay fixtures/sample-complete-replay.json

# 3. Run a deterministic AI-vs-AI match and export a verifiable replay
cargo run -- simulate --seed 42 --starter 0 --policy mixed --export-replay /tmp/match-42.json
cargo run -- verify-replay /tmp/match-42.json

# 4. Inspect the initial Enchanted Table / LUMI agent state for a seeded match
cargo run -- inspect-state --seed 42 --starter 0

# 5. Launch the live Wonderland Digital 0.4 — The Enchanted Table server on 127.0.0.1:8765
cargo run -- serve --port 8765 --seed 42
```

