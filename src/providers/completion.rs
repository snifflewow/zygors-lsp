use tower_lsp::lsp_types::*;

use crate::db::index::Database;
use crate::guide::types::GuideModel;

const ACTION_KEYWORDS: &[&str] = &[
    "accept", "turnin", "talk", "kill", "collect", "click", "clicknpc", "use", "buy", "get",
    "goal", "trash", "destroy", "bank", "equip", "learnspell", "learn", "cast", "ding", "fpath",
    "home", "hearth", "info", "tip", "confirm", "stickystart", "stickystop", "label",
    "stickyif", "step", "map", "title", "level", "path", "from", "vendor", "trainer", "mine",
];

const PIPE_DIRECTIVES: &[&str] = &[
    "goto", "q", "only", "c", "n", "future", "instant", "killcount", "noway", "notravel",
    "nohearth", "walk", "zombiewalk", "noautoaccept", "noobsolete", "daily", "notinsticky",
    "noordinal", "sticky", "gossip", "confirm", "complete", "condition", "count", "or", "tip",
    "title", "popuptext", "next", "from", "script", "vendor", "talk", "havebuff", "nobuff",
    "skillmax",
];

pub fn complete(
    line_text: &str,
    db: &Database,
    model: Option<&GuideModel>,
) -> Option<CompletionResponse> {
    let trimmed = line_text.trim_start_matches('.');

    if let Some(after_pipe) = trimmed.strip_prefix('|') {
        return Some(complete_pipe_directive(after_pipe));
    }

    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    let after_keyword = trimmed
        .strip_prefix(first_word)
        .unwrap_or("")
        .trim_start();

    match first_word {
        "accept" | "turnin" => {
            return Some(complete_entities(&db.quest_names, after_keyword, "quest", db));
        }
        "talk" | "kill" | "clicknpc" | "from" => {
            return Some(complete_entities(&db.unit_names, after_keyword, "unit", db));
        }
        "collect" | "use" | "buy" | "trash" | "destroy" | "bank" | "equip" | "equipped"
        | "mine" => {
            return Some(complete_entities(&db.item_names, after_keyword, "item", db));
        }
        "click" => {
            return Some(complete_entities(
                &db.object_names,
                after_keyword,
                "object",
                db,
            ));
        }
        "map" => {
            return Some(complete_zones(&db.zone_names, after_keyword));
        }
        "label" => {
            if let Some(model) = model {
                return Some(complete_labels(model, after_keyword));
            }
        }
        "" => {
            return Some(complete_action_keywords(""));
        }
        _ => {}
    }

    if !after_keyword.is_empty() {
        return None;
    }

    if ACTION_KEYWORDS.contains(&first_word) {
        return None;
    }

    Some(complete_action_keywords(first_word))
}

fn complete_action_keywords(prefix: &str) -> CompletionResponse {
    let prefix_lower = prefix.to_lowercase();
    let items: Vec<CompletionItem> = ACTION_KEYWORDS
        .iter()
        .filter(|kw| kw.starts_with(&prefix_lower))
        .map(|kw| CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        })
        .collect();
    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

fn complete_pipe_directive(prefix: &str) -> CompletionResponse {
    let prefix_lower = prefix.to_lowercase();
    let items: Vec<CompletionItem> = PIPE_DIRECTIVES
        .iter()
        .filter(|d| d.starts_with(&prefix_lower))
        .map(|d| CompletionItem {
            label: d.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            ..Default::default()
        })
        .collect();
    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

fn complete_entities(
    names: &[(String, u32)],
    prefix: &str,
    kind_label: &str,
    db: &Database,
) -> CompletionResponse {
    let prefix_lower = prefix.to_lowercase();
    let count_prefix = prefix
        .split_whitespace()
        .next()
        .and_then(|w| w.parse::<u32>().ok());
    let search_prefix = if count_prefix.is_some() {
        prefix
            .strip_prefix(prefix.split_whitespace().next().unwrap_or(""))
            .unwrap_or("")
            .trim_start()
            .to_lowercase()
    } else {
        prefix_lower
    };

    let items: Vec<CompletionItem> = names
        .iter()
        .filter(|(name, _)| name.to_lowercase().starts_with(&search_prefix))
        .take(50)
        .map(|(name, id)| {
            let (detail, insert_text) = match kind_label {
                "quest" => (
                    db.quests
                        .get(id)
                        .map(|q| format!("Quest [{}] (lvl {})", id, q.level)),
                    format!("{}##{}", name, id),
                ),
                "unit" => {
                    let detail = db
                        .units
                        .get(id)
                        .map(|u| format!("NPC [{}] (lvl {})", id, u.level));
                    let text = build_insert_with_goto_unit(name, *id, db);
                    (detail, text)
                }
                "object" => {
                    let detail = Some(format!("Object [{}]", id));
                    let text = build_insert_with_goto_object(name, *id, db);
                    (detail, text)
                }
                "item" => (Some(format!("Item [{}]", id)), format!("{}##{}", name, id)),
                _ => (None, format!("{}##{}", name, id)),
            };
            CompletionItem {
                label: name.clone(),
                kind: Some(CompletionItemKind::VALUE),
                detail,
                insert_text: Some(insert_text),
                ..Default::default()
            }
        })
        .collect();

    CompletionResponse::List(CompletionList {
        is_incomplete: items.len() >= 50,
        items,
    })
}

fn build_insert_with_goto_unit(name: &str, id: u32, db: &Database) -> String {
    if let Some(unit) = db.units.get(&id) {
        if let Some(coord) = unit.coords.first() {
            if let Some(zone) = db.zones.get(&coord.zone_id) {
                return format!(
                    "{}##{}  |goto {} {:.2},{:.2}",
                    name, id, zone.name, coord.x, coord.y
                );
            }
        }
    }
    format!("{}##{}", name, id)
}

fn build_insert_with_goto_object(name: &str, id: u32, db: &Database) -> String {
    if let Some(obj) = db.objects.get(&id) {
        if let Some(coord) = obj.coords.first() {
            if let Some(zone) = db.zones.get(&coord.zone_id) {
                return format!(
                    "{}##{}  |goto {} {:.2},{:.2}",
                    name, id, zone.name, coord.x, coord.y
                );
            }
        }
    }
    format!("{}##{}", name, id)
}

fn complete_zones(names: &[(String, u32)], prefix: &str) -> CompletionResponse {
    let prefix_lower = prefix.to_lowercase();
    let items: Vec<CompletionItem> = names
        .iter()
        .filter(|(name, _)| name.to_lowercase().starts_with(&prefix_lower))
        .take(50)
        .map(|(name, id)| CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some(format!("Zone [{}]", id)),
            ..Default::default()
        })
        .collect();
    CompletionResponse::List(CompletionList {
        is_incomplete: items.len() >= 50,
        items,
    })
}

fn complete_labels(model: &GuideModel, prefix: &str) -> CompletionResponse {
    let prefix_clean = prefix.trim_matches('"');
    let items: Vec<CompletionItem> = model
        .sticky_starts
        .keys()
        .filter(|name| name.starts_with(prefix_clean))
        .map(|name| CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::REFERENCE),
            insert_text: Some(format!("\"{}\"", name)),
            ..Default::default()
        })
        .collect();
    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_db() -> Database {
        Database {
            quests: HashMap::new(),
            units: HashMap::new(),
            items: HashMap::new(),
            objects: HashMap::new(),
            zones: HashMap::new(),
            quest_names: Vec::new(),
            unit_names: Vec::new(),
            item_names: Vec::new(),
            object_names: Vec::new(),
            zone_names: vec![
                ("Elwynn Forest".to_string(), 12),
                ("Durotar".to_string(), 14),
                ("Darkshore".to_string(), 148),
            ],
            zone_name_to_id: HashMap::new(),
        }
    }

    #[test]
    fn test_action_keyword_completion() {
        let db = empty_db();
        let result = complete("ac", &db, None).unwrap();
        let items = match result {
            CompletionResponse::List(l) => l.items,
            _ => panic!("expected list"),
        };
        assert!(items.iter().any(|i| i.label == "accept"));
    }

    #[test]
    fn test_pipe_completion() {
        let db = empty_db();
        let result = complete("|go", &db, None).unwrap();
        let items = match result {
            CompletionResponse::List(l) => l.items,
            _ => panic!("expected list"),
        };
        assert!(items.iter().any(|i| i.label == "goto"));
    }

    #[test]
    fn test_zone_completion() {
        let db = empty_db();
        let result = complete("map D", &db, None).unwrap();
        let items = match result {
            CompletionResponse::List(l) => l.items,
            _ => panic!("expected list"),
        };
        assert!(items.iter().any(|i| i.label == "Durotar"));
        assert!(items.iter().any(|i| i.label == "Darkshore"));
        assert!(!items.iter().any(|i| i.label == "Elwynn Forest"));
    }
}
