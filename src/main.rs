use clap::{Parser, Subcommand};
use heav3ns_engine::{
    adapter::{pool_sha256, stock_game, Match},
    contract::{snapshot, verify_replay_data},
    engine::{load_proposed_specs, specs_hash, DecisionProviders},
    policy::AuditPolicy,
    server::serve,
};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "heav3ns-engine",
    version = "0.4.1",
    about = "HEAV3NS-ENGINE — Dedicated deterministic Rust runtime & Enchanted Table server for WONDERLAND"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch the local Enchanted Table HTTP API & LUMI agent bridge server (/api/* endpoints)
    Serve {
        #[arg(long, default_value_t = 8765)]
        port: u16,
        #[arg(long, default_value_t = 42)]
        seed: i64,
        #[arg(long, default_value = "assets")]
        static_root: PathBuf,
    },
    /// Verify a wonderland-digital-replay-v1 JSON file against the deterministic Rust engine
    VerifyReplay {
        /// Path to the replay JSON artifact
        path: PathBuf,
    },
    /// Run a deterministic AI-vs-AI simulation match using AuditPolicy or heuristic Policy
    Simulate {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 0)]
        starter: usize,
        #[arg(long, default_value_t = 80)]
        max_turns: usize,
        #[arg(long, default_value = "mixed")]
        policy: String,
        #[arg(long, default_value_t = false)]
        heuristic: bool,
        #[arg(long)]
        export_replay: Option<PathBuf>,
    },
    /// Inspect seeded match state, legal actions, and redacted agent view (LUMI / agent bridge)
    InspectState {
        #[arg(long, default_value_t = 42)]
        seed: i64,
        #[arg(long, default_value_t = 0)]
        starter: i64,
    },
    /// Run a complete infrastructure & baseline integrity audit
    Audit,
}

fn run_simulation(
    seed: u64,
    starter: usize,
    max_turns: usize,
    policy_name: &str,
    heuristic: bool,
    export_replay: Option<PathBuf>,
) -> Result<Value, String> {
    let mut g = stock_game(seed)?;
    let initial = snapshot(&g)?;
    g.trace_transitions = true;
    let mut ai = if heuristic {
        AuditPolicy::new_heuristic([policy_name, policy_name])
    } else {
        AuditPolicy::new([policy_name, policy_name])
    };
    for turn in 0..max_turns {
        let owner = (starter + turn) % 2;
        g.start_with_providers(owner, &ai).map_err(|e| e.0)?;
        while g.winner.is_none() {
            let chosen = ai.sequence(&g, owner);
            if chosen.is_empty() {
                if g.phase == "placement" {
                    g.close_with_providers(&mut ai).map_err(|e| e.0)?;
                }
                break;
            }
            if g.phase == "resolved" {
                g.begin_sequence_with_providers(&ai).map_err(|e| e.0)?;
            }
            for c in chosen {
                g.place_with_providers(c.uid, &ai).map_err(|e| e.0)?;
            }
            g.close_with_providers(&mut ai).map_err(|e| e.0)?;
        }
        let excess = g.p[owner].hand.len().saturating_sub(6);
        let discard = if g.winner.is_some() || excess == 0 {
            Vec::new()
        } else {
            ai.discard(&g, owner, excess).map_err(|e| e.0)?
        };
        g.end_turn_with_providers(&discard, &ai).map_err(|e| e.0)?;
        g.invariants().map_err(|e| e.0)?;
        if g.winner.is_some() {
            break;
        }
    }
    if let Some(ref path) = export_replay {
        let replay_doc = json!({
            "schema": "wonderland-digital-replay-v1",
            "seed": seed,
            "starter": starter,
            "pool_sha256": pool_sha256(),
            "runtime_sha256": heav3ns_engine::adapter::runtime_hashes(),
            "initial_state": initial,
            "inputs": [],
            "transitions": g.transitions,
            "log": g.log,
            "winner": g.winner,
            "error": Value::Null
        });
        std::fs::write(
            path,
            serde_json::to_string_pretty(&replay_doc).unwrap() + "\n",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(json!({
        "ok": true,
        "seed": seed,
        "starter": starter,
        "policy": policy_name,
        "mode": if heuristic { "heuristic" } else { "audit" },
        "turns": g.turn_number,
        "winner": g.winner,
        "winner_name": g.winner.map(|w| if w == 0 { "Alice" } else { "White Rabbit" }),
        "transitions": g.transitions.len(),
        "events": g.log.len(),
        "shields_remaining": [g.p[0].shields.len(), g.p[1].shields.len()]
    }))
}

fn main() {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Serve {
        port: 8765,
        seed: 42,
        static_root: PathBuf::from("assets"),
    }) {
        Commands::Serve {
            port,
            seed,
            static_root,
        } => {
            if let Err(e) = serve(port, seed, static_root) {
                eprintln!("Server error: {e}");
                std::process::exit(1);
            }
        }
        Commands::VerifyReplay { path } => {
            let res: Result<Value, String> = (|| {
                let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let data: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
                verify_replay_data(&data)
            })();
            match res {
                Ok(summary) => {
                    println!("{}", serde_json::to_string(&summary).unwrap());
                }
                Err(e) => {
                    println!(
                        "{}",
                        serde_json::to_string(&json!({"ok": false, "error": e})).unwrap()
                    );
                    std::process::exit(1);
                }
            }
        }
        Commands::Simulate {
            seed,
            starter,
            max_turns,
            policy,
            heuristic,
            export_replay,
        } => match run_simulation(seed, starter, max_turns, &policy, heuristic, export_replay) {
            Ok(summary) => {
                println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            }
            Err(e) => {
                eprintln!("Simulation failed: {e}");
                std::process::exit(1);
            }
        },
        Commands::InspectState { seed, starter } => match Match::new(seed, starter) {
            Ok(m) => {
                // Wait briefly for worker thread to reach first human decision or AI step
                for _ in 0..40 {
                    let st = m.state();
                    if st.get("status").and_then(|v| v.as_str()) == Some("waiting") {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                let st = m.state();
                m.stop();
                println!("{}", serde_json::to_string_pretty(&st).unwrap());
            }
            Err(e) => {
                eprintln!("InspectState failed: {e}");
                std::process::exit(1);
            }
        },
        Commands::Audit => {
            let specs = load_proposed_specs();
            let p_hash = pool_sha256();
            let s_hash = specs_hash(&specs);
            let mut replay_checks = Vec::new();
            for fixture in [
                "fixtures/sample-complete-replay.json",
                "fixtures/downloaded-seed2-replay.json",
                "fixtures/downloaded-seed2-final.json",
            ] {
                let p = PathBuf::from(fixture);
                if p.is_file() {
                    let raw = std::fs::read_to_string(&p).unwrap();
                    let val: Value = serde_json::from_str(&raw).unwrap();
                    let res = verify_replay_data(&val).unwrap();
                    replay_checks.push(json!({
                        "fixture": fixture,
                        "result": res
                    }));
                }
            }
            let report = json!({
                "engine": "HEAV3NS-ENGINE",
                "version": "0.4.1",
                "product": "WONDERLAND (FC04.1 / Digital 0.4 Enchanted Table)",
                "card_definitions": specs.len(),
                "pool_sha256": p_hash,
                "specs_sha256": s_hash,
                "historical_replays_verified": replay_checks
            });
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        }
    }
}
