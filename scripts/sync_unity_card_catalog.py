#!/usr/bin/env python3
"""Keep Unity's Resources catalog aligned with the Rust engine's proposed card specs."""
import argparse
import json
from pathlib import Path

root = Path(__file__).resolve().parent.parent
source = root / "data/proposed.json"
target = root / "unity_client/Assets/Resources/Hub/CardCatalog.json"
fields = ("id", "title", "deck", "copies", "cost", "type", "text")
cards = json.loads(source.read_text(encoding="utf-8"))
expected = json.dumps({"cards": [{key: card[key] for key in fields} for card in cards]}, ensure_ascii=False, indent=2)
parser = argparse.ArgumentParser()
parser.add_argument("--check", action="store_true", help="fail if the checked-in catalog is stale")
args = parser.parse_args()
if args.check:
    if not target.exists() or target.read_text(encoding="utf-8") != expected:
        raise SystemExit("Unity card catalog is stale; run scripts/sync_unity_card_catalog.py")
    print(f"Unity catalog matches {len(cards)} Rust card specs")
else:
    target.write_text(expected, encoding="utf-8")
    print(f"Updated {target} with {len(cards)} cards")
