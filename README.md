# HEAV3NS-ENGINE

**HEAV3NS-ENGINE** is the dedicated, deterministic Rust game engine and local visual runtime built exclusively for **WONDERLAND** (*Wonderland Digital 0.3* and *Wonderland Digital 0.4 — The Enchanted Table*).

> **WONDERLAND is the product. HEAV3NS-ENGINE is the machinery built to realize it.**

## Quick Start

### 1. Run the Test Suite
```bash
cargo test
```

### 2. Run the Baseline & Historical Replay Audit
```bash
cargo run -- audit
```
Verifies the canonical FC04.1 44-card pool (`data/proposed.json`, SHA-256 `db4fb5bad7c274cc77a5323e44651dc427a17ba67a9e80e47706000fbc5517c2`), specs hash (`4a290f53da32379b4f0ff87ee988cd7ad142944d461f09724c87d46626661704`), and bit-exact replay reproduction of the historical Wonderland 0.2/0.3/0.4 match fixtures (`fixtures/sample-complete-replay.json`, `fixtures/downloaded-seed2-replay.json`, and `fixtures/downloaded-seed2-final.json`).

### 3. Launch Wonderland Digital 0.4 — The Enchanted Table (Macroquad 0.4 Client)
```bash
cargo run -p client_macroquad --release -- --seed 42
```
Launches the native 1920x1080 60+ FPS **Macroquad (`v0.4.x`)** interactive puppet-stage client (`client_macroquad`) against the headless deterministic `heav3ns-engine` backend.
(You can also run the headless API/audit server via `cargo run -p heav3ns-engine -- serve --port 8765 --seed 42`.)

### 4. Simulate Matches & Verify Replays
```bash
# Run a full deterministic AI-vs-AI match and export a replay
cargo run -- simulate --seed 42 --starter 0 --policy mixed --export-replay /tmp/replay.json

# Verify the replay against the deterministic transition verifier
cargo run -- verify-replay /tmp/replay.json
```

## Architecture & Commission Deliverables

See [`ARCHITECTURE.md`](file:///Users/bozoegg/Desktop/HEAV3NS-ENGINE/ARCHITECTURE.md) for the complete First-Phase Commission Deliverables:
1. **Wonderland Infrastructure Audit**
2. **Engine Architecture Specification**
3. **Existing Implementation Migration Plan**
4. **Rules and Simulation Specification**
5. **Visual Runtime Specification**
6. **Agent Integration Specification (LUMI / BroccoliDB / BroccoliQueue)**
7. **Implementation Roadmap**
8. **Technical Demonstrator**

