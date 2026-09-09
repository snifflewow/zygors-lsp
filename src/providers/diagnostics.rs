use tower_lsp::lsp_types::*;

use super::position::byte_range_to_lsp;
use crate::db::index::Database;
use crate::guide::types::{EntityKind, GuideModel};

pub fn diagnose(
    model: &GuideModel,
    db: &Database,
    source: &str,
    content_offset: usize,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    check_entity_refs(model, db, source, content_offset, &mut diags);
    check_quest_refs(model, db, source, content_offset, &mut diags);
    check_goto_refs(model, db, source, content_offset, &mut diags);
    check_sticky_labels(model, source, content_offset, &mut diags);

    diags
}

fn check_entity_refs(
    model: &GuideModel,
    db: &Database,
    source: &str,
    content_offset: usize,
    diags: &mut Vec<Diagnostic>,
) {
    for eref in &model.entity_refs {
        let id = eref.id;
        let range = byte_range_to_lsp(source, content_offset, &eref.byte_range);

        if eref.kind == EntityKind::Spell {
            continue;
        }

        let in_expected = exists_in_table(db, id, eref.kind);

        if in_expected {
            let db_name = get_entity_name(db, id, eref.kind);
            if let Some(db_name) = db_name {
                if !db_name.is_empty() && db_name != eref.name {
                    diags.push(Diagnostic {
                        range,
                        severity: Some(DiagnosticSeverity::WARNING),
                        source: Some("zygorguide".into()),
                        message: format!(
                            "Name mismatch: guide says '{}', database says '{}'",
                            eref.name, db_name
                        ),
                        ..Default::default()
                    });
                }
            }
            continue;
        }

        if eref.kind == EntityKind::Unknown {
            if exists_in_any_table(db, id) {
                continue;
            }
            diags.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("zygorguide".into()),
                message: format!("Unknown entity ID {}", id),
                ..Default::default()
            });
            continue;
        }

        let found_elsewhere = find_entity_type_excluding(db, id, eref.kind);
        if let Some(actual) = found_elsewhere {
            let expected_str = kind_label(eref.kind);
            let actual_str = kind_label(actual);
            diags.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                source: Some("zygorguide".into()),
                message: format!(
                    "ID {} is a {}, but used as {} (action: {})",
                    id, actual_str, expected_str, eref.action
                ),
                ..Default::default()
            });
        } else {
            diags.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("zygorguide".into()),
                message: format!("Unknown entity ID {}", id),
                ..Default::default()
            });
        }
    }
}

fn check_quest_refs(
    model: &GuideModel,
    db: &Database,
    source: &str,
    content_offset: usize,
    diags: &mut Vec<Diagnostic>,
) {
    for qref in &model.quest_refs {
        let range = byte_range_to_lsp(source, content_offset, &qref.byte_range);

        if db.quests.get(&qref.quest_id).is_none() {
            diags.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("zygorguide".into()),
                message: format!("Unknown quest ID {} in |q directive", qref.quest_id),
                ..Default::default()
            });
        }
    }
}

fn check_goto_refs(
    model: &GuideModel,
    db: &Database,
    source: &str,
    content_offset: usize,
    diags: &mut Vec<Diagnostic>,
) {
    for goto in &model.goto_refs {
        let range = byte_range_to_lsp(source, content_offset, &goto.byte_range);

        if let Some(zone) = &goto.zone {
            if db.lookup_zone_by_name(zone).is_none() {
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("zygorguide".into()),
                    message: format!("Unknown zone '{}'", zone),
                    ..Default::default()
                });
            }
        }

        if let (Some(x), Some(y)) = (goto.x, goto.y) {
            if x < 0.0 || x > 100.0 || y < 0.0 || y > 100.0 {
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("zygorguide".into()),
                    message: format!(
                        "Coordinate ({:.1}, {:.1}) out of range — expected 0-100",
                        x, y
                    ),
                    ..Default::default()
                });
            }
        }
    }
}

fn check_sticky_labels(
    model: &GuideModel,
    source: &str,
    content_offset: usize,
    diags: &mut Vec<Diagnostic>,
) {
    for (name, _step_idx) in &model.sticky_starts {
        if !model.labels.contains_key(name) && !model.sticky_stops.contains_key(name) {
            if let Some(lref) = model
                .label_refs
                .iter()
                .find(|r| r.name == *name)
            {
                let range = byte_range_to_lsp(source, content_offset, &lref.byte_range);
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("zygorguide".into()),
                    message: format!("Unmatched stickystart '{}' — no label or stickystop", name),
                    ..Default::default()
                });
            }
        }
    }

    for (name, _step_indices) in &model.labels {
        if !model.sticky_starts.contains_key(name) {
            if let Some(lref) = model
                .label_refs
                .iter()
                .find(|r| r.name == *name)
            {
                let range = byte_range_to_lsp(source, content_offset, &lref.byte_range);
                diags.push(Diagnostic {
                    range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("zygorguide".into()),
                    message: format!("Label '{}' has no matching stickystart", name),
                    ..Default::default()
                });
            }
        }
    }
}

fn exists_in_table(db: &Database, id: u32, kind: EntityKind) -> bool {
    match kind {
        EntityKind::Quest => db.quests.contains_key(&id),
        EntityKind::Unit => db.units.contains_key(&id),
        EntityKind::Item => db.items.contains_key(&id),
        EntityKind::Object => db.objects.contains_key(&id),
        EntityKind::Spell | EntityKind::Unknown => false,
    }
}

fn exists_in_any_table(db: &Database, id: u32) -> bool {
    db.quests.contains_key(&id)
        || db.units.contains_key(&id)
        || db.items.contains_key(&id)
        || db.objects.contains_key(&id)
}

fn find_entity_type_excluding(db: &Database, id: u32, exclude: EntityKind) -> Option<EntityKind> {
    if exclude != EntityKind::Quest && db.quests.contains_key(&id) {
        return Some(EntityKind::Quest);
    }
    if exclude != EntityKind::Unit && db.units.contains_key(&id) {
        return Some(EntityKind::Unit);
    }
    if exclude != EntityKind::Item && db.items.contains_key(&id) {
        return Some(EntityKind::Item);
    }
    if exclude != EntityKind::Object && db.objects.contains_key(&id) {
        return Some(EntityKind::Object);
    }
    None
}

fn get_entity_name(db: &Database, id: u32, kind: EntityKind) -> Option<String> {
    match kind {
        EntityKind::Quest => db.quests.get(&id).map(|q| q.title.clone()),
        EntityKind::Unit => db.units.get(&id).map(|u| u.name.clone()),
        EntityKind::Item => db.items.get(&id).map(|i| i.name.clone()),
        EntityKind::Object => db.objects.get(&id).map(|o| o.name.clone()),
        _ => None,
    }
}

fn kind_label(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Quest => "quest",
        EntityKind::Unit => "unit/NPC",
        EntityKind::Item => "item",
        EntityKind::Object => "object",
        EntityKind::Spell => "spell",
        EntityKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::parser::DocumentParser;
    use crate::guide::semantic;
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
            zone_names: Vec::new(),
            zone_name_to_id: HashMap::new(),
        }
    }

    fn db_with_quest(id: u32, title: &str) -> Database {
        let mut db = empty_db();
        db.quests.insert(
            id,
            crate::db::types::Quest {
                id,
                title: title.to_string(),
                objective_text: String::new(),
                description: String::new(),
                level: 1,
                min_level: 1,
                start_units: vec![],
                start_objects: vec![],
                start_items: vec![],
                end_units: vec![],
                end_objects: vec![],
                obj_units: vec![],
                obj_items: vec![],
                obj_objects: vec![],
                pre: vec![],
                next: vec![],
                close: vec![],
                race: 0,
                class: 0,
            },
        );
        db
    }

    fn parse_and_diagnose(guide_content: &str, db: &Database) -> Vec<Diagnostic> {
        let source = format!(
            "ZygorGuidesViewer:RegisterGuide('Test', {{}}, [[{}]])",
            guide_content
        );
        let mut parser = DocumentParser::new();
        let regions = parser.parse_document(&source);
        assert_eq!(regions.len(), 1);
        let model = semantic::build_model(&regions[0]);
        diagnose(&model, db, &source, regions[0].content_offset)
    }

    #[test]
    fn test_unknown_entity_id() {
        let db = empty_db();
        let diags = parse_and_diagnose(
            "\nstep\naccept Fake Quest##99999\n",
            &db,
        );
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("Unknown entity ID 99999"));
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn test_valid_quest_no_diagnostic() {
        let db = db_with_quest(783, "A Threat Within");
        let diags = parse_and_diagnose(
            "\nstep\naccept A Threat Within##783\n",
            &db,
        );
        let entity_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Unknown entity"))
            .collect();
        assert!(entity_errors.is_empty());
    }

    #[test]
    fn test_name_mismatch() {
        let db = db_with_quest(783, "A Threat Within");
        let diags = parse_and_diagnose(
            "\nstep\naccept Wrong Name##783\n",
            &db,
        );
        let mismatches: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Name mismatch"))
            .collect();
        assert_eq!(mismatches.len(), 1);
        assert!(mismatches[0].message.contains("Wrong Name"));
        assert!(mismatches[0].message.contains("A Threat Within"));
    }

    #[test]
    fn test_wrong_entity_type() {
        let mut db = empty_db();
        db.units.insert(
            783,
            crate::db::types::Unit {
                id: 783,
                name: "Some NPC".to_string(),
                coords: vec![],
                faction: None,
                level: String::new(),
            },
        );
        let diags = parse_and_diagnose(
            "\nstep\naccept Some NPC##783\n",
            &db,
        );
        let type_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("is a"))
            .collect();
        assert_eq!(type_errors.len(), 1);
        assert!(type_errors[0].message.contains("unit/NPC"));
        assert!(type_errors[0].message.contains("quest"));
    }

    #[test]
    fn test_unknown_quest_pipe() {
        let db = empty_db();
        let diags = parse_and_diagnose(
            "\nstep\naccept Test##1 |q 99999/1\n",
            &db,
        );
        let quest_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Unknown quest ID"))
            .collect();
        assert!(!quest_errors.is_empty());
    }

    #[test]
    fn test_unmatched_stickystart() {
        let db = empty_db();
        let diags = parse_and_diagnose(
            "\nstep\nstickystart \"Orphan\"\nkill Something##1\n",
            &db,
        );
        let sticky_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Unmatched stickystart"))
            .collect();
        assert_eq!(sticky_errors.len(), 1);
        assert!(sticky_errors[0].message.contains("Orphan"));
    }

    #[test]
    fn test_matched_stickystart_no_error() {
        let db = empty_db();
        let diags = parse_and_diagnose(
            "\nstep\nstickystart \"Paired\"\nkill Something##1\nstep\nlabel \"Paired\"\ncollect Stuff##2\n",
            &db,
        );
        let sticky_errors: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("Unmatched stickystart"))
            .collect();
        assert!(sticky_errors.is_empty());
    }

    #[test]
    #[ignore]
    fn test_diagnose_real_guide() {
        use std::path::Path;

        let pf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../pfQuest");
        let epoch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../pfQuest-epoch");
        let db = crate::db::index::Database::load(&pf, &epoch).expect("load db");

        let guide_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ZygorGuidesViewer-EpochReborn/Guides/EPOCH/1-6Orc.lua");
        let source = std::fs::read_to_string(&guide_path).expect("read guide");

        let mut parser = DocumentParser::new();
        let regions = parser.parse_document(&source);
        assert!(!regions.is_empty(), "should find RegisterGuide calls");

        let mut total_diags = 0;
        for region in &regions {
            let model = semantic::build_model(region);
            let diags = diagnose(&model, &db, &source, region.content_offset);
            for d in &diags {
                let sev = match d.severity {
                    Some(DiagnosticSeverity::ERROR) => "ERROR",
                    Some(DiagnosticSeverity::WARNING) => "WARN",
                    _ => "INFO",
                };
                println!(
                    "  [{}] L{}:{} {}",
                    sev,
                    d.range.start.line + 1,
                    d.range.start.character,
                    d.message
                );
            }
            total_diags += diags.len();
            println!(
                "Guide '{}': {} entity_refs, {} quest_refs, {} goto_refs, {} diagnostics",
                model.title,
                model.entity_refs.len(),
                model.quest_refs.len(),
                model.goto_refs.len(),
                diags.len()
            );
        }
        println!("Total diagnostics across all guides: {}", total_diags);
    }

    #[test]
    fn test_byte_offset_to_position() {
        use crate::providers::position::byte_offset_to_position;
        let source = "line one\nline two\nline three";
        assert_eq!(byte_offset_to_position(source, 0), Position { line: 0, character: 0 });
        assert_eq!(byte_offset_to_position(source, 5), Position { line: 0, character: 5 });
        assert_eq!(byte_offset_to_position(source, 9), Position { line: 1, character: 0 });
        assert_eq!(byte_offset_to_position(source, 14), Position { line: 1, character: 5 });
        assert_eq!(byte_offset_to_position(source, 18), Position { line: 2, character: 0 });
    }
}
