use heav3ns_engine::adapter::{custom_game, custom_game_for_stage};
use heav3ns_engine::engine::load_proposed_specs;
use serde_json::Value;

fn starter(faction: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for card in load_proposed_specs() {
        if card.get("deck").and_then(Value::as_str) != Some(faction) { continue; }
        let id = card["id"].as_str().unwrap();
        for _ in 0..card["copies"].as_u64().unwrap() {
            if ids.len() < 30 { ids.push(id.to_owned()); }
        }
    }
    ids
}

#[test]
fn custom_decks_are_validated_and_dealt_without_changing_stock_format() {
    for faction in ["Alice", "White Rabbit"] {
        let ids = starter(faction);
        let (game, names) = custom_game(42, &ids).unwrap();
        assert_eq!(names[0], faction);
        for player in &game.p {
            assert_eq!((player.shields.len(), player.hand.len(), player.deck.len()), (6, 5, 19));
        }
        let (again, _) = custom_game(42, &ids).unwrap();
        let a: Vec<_> = game.p[0].hand.iter().map(|c| c.id()).collect();
        let b: Vec<_> = again.p[0].hand.iter().map(|c| c.id()).collect();
        assert_eq!(a, b);
    }
    let mut short = starter("Alice"); short.pop();
    assert!(custom_game(42, &short).is_err());
    let mut mixed = starter("Alice"); mixed[0] = starter("White Rabbit")[0].clone();
    assert!(custom_game(42, &mixed).is_err());
    let mut excessive = starter("Alice"); excessive[29] = excessive[0].clone();
    assert!(custom_game(42, &excessive).is_err());
    assert!(custom_game(2_147_483_648, &starter("Alice")).is_err());
    assert!(custom_game_for_stage(42, &starter("Alice"), 4).is_err());
    let (normal, _) = custom_game_for_stage(42, &starter("Alice"), 1).unwrap();
    let (boss, _) = custom_game_for_stage(42, &starter("Alice"), 3).unwrap();
    let total_cost = |game: &heav3ns_engine::engine::Game| game.p[1].hand.iter().chain(&game.p[1].deck).chain(&game.p[1].shields).map(|c| c.cost()).sum::<i32>();
    assert!(total_cost(&boss) > total_cost(&normal), "boss stage should use its distinct high-cost opponent deck");
}
