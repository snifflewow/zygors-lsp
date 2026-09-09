use tower_lsp::lsp_types::*;

use crate::db::index::Database;
use crate::guide::types::{EntityKind, GuideModel};
use crate::providers::position::byte_range_to_lsp;

pub fn code_actions(
    model: &GuideModel,
    db: &Database,
    source: &str,
    content_offset: usize,
    uri: &Url,
    request_range: Range,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for eref in &model.entity_refs {
        let ref_range = byte_range_to_lsp(source, content_offset, &eref.byte_range);
        if !ranges_overlap(&ref_range, &request_range) {
            continue;
        }

        if let Some(action) =
            make_add_goto_action(eref, model, db, source, content_offset, uri, ref_range)
        {
            actions.push(action);
        }

        let db_name = match eref.kind {
            EntityKind::Quest => db.quests.get(&eref.id).map(|q| q.title.as_str()),
            EntityKind::Unit => db.units.get(&eref.id).map(|u| u.name.as_str()),
            EntityKind::Item => db.items.get(&eref.id).map(|i| i.name.as_str()),
            EntityKind::Object => db.objects.get(&eref.id).map(|o| o.name.as_str()),
            EntityKind::Unknown => db
                .quests
                .get(&eref.id)
                .map(|q| q.title.as_str())
                .or_else(|| db.units.get(&eref.id).map(|u| u.name.as_str()))
                .or_else(|| db.items.get(&eref.id).map(|i| i.name.as_str()))
                .or_else(|| db.objects.get(&eref.id).map(|o| o.name.as_str())),
            EntityKind::Spell => None,
        };

        if let Some(db_name) = db_name {
            if !db_name.is_empty() && db_name != eref.name {
                let new_text = format!("{}##{}", db_name, eref.id);
                let edit = TextEdit {
                    range: ref_range,
                    new_text: new_text.clone(),
                };

                let mut changes = std::collections::HashMap::new();
                changes.insert(uri.clone(), vec![edit]);

                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: format!("Fix name to '{}'", db_name),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: None,
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        ..Default::default()
                    }),
                    ..Default::default()
                }));
            }
        }

        if eref.kind != EntityKind::Spell && eref.kind != EntityKind::Unknown {
            let exists_in_expected = exists_in_table(db, eref.id, eref.kind);
            if !exists_in_expected {
                if let Some((suggested_name, suggested_id)) =
                    find_closest_by_name(db, &eref.name, eref.kind)
                {
                    let new_text = format!("{}##{}", suggested_name, suggested_id);
                    let edit = TextEdit {
                        range: ref_range,
                        new_text,
                    };

                    let mut changes = std::collections::HashMap::new();
                    changes.insert(uri.clone(), vec![edit]);

                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: format!("Did you mean '{}' (ID {})?", suggested_name, suggested_id),
                        kind: Some(CodeActionKind::QUICKFIX),
                        edit: Some(WorkspaceEdit {
                            changes: Some(changes),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }));
                }
            }
        }
    }

    actions
}

fn ranges_overlap(a: &Range, b: &Range) -> bool {
    a.start.line <= b.end.line && b.start.line <= a.end.line
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

fn make_add_goto_action(
    eref: &crate::guide::types::EntityRef,
    model: &GuideModel,
    db: &Database,
    source: &str,
    content_offset: usize,
    uri: &Url,
    ref_range: Range,
) -> Option<CodeActionOrCommand> {
    let has_coords = matches!(
        eref.action.as_str(),
        "talk" | "kill" | "click" | "clicknpc" | "vendor" | "trainer"
    );
    if !has_coords {
        return None;
    }

    let line_start_byte = content_offset + eref.byte_range.start;
    let line_start = source[..line_start_byte].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = source[line_start_byte..]
        .find('\n')
        .map(|i| line_start_byte + i)
        .unwrap_or(source.len());
    let line_text = &source[line_start..line_end];

    if line_text.contains("|goto") || line_text.contains("|at") {
        return None;
    }

    let goto_text = match eref.kind {
        EntityKind::Unit => {
            let unit = db.units.get(&eref.id)?;
            let coord = unit.coords.first()?;
            let zone = db.zones.get(&coord.zone_id)?;
            format!("|goto {} {:.2},{:.2}", zone.name, coord.x, coord.y)
        }
        EntityKind::Object => {
            let obj = db.objects.get(&eref.id)?;
            let coord = obj.coords.first()?;
            let zone = db.zones.get(&coord.zone_id)?;
            format!("|goto {} {:.2},{:.2}", zone.name, coord.x, coord.y)
        }
        _ => return None,
    };

    let insert_pos = Position {
        line: ref_range.start.line,
        character: (line_end - line_start) as u32,
    };

    let edit = TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: format!("  {}", goto_text),
    };

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri.clone(), vec![edit]);

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("Add {}", goto_text),
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        ..Default::default()
    }))
}

fn find_closest_by_name(
    db: &Database,
    name: &str,
    kind: EntityKind,
) -> Option<(String, u32)> {
    let name_lower = name.to_lowercase();
    let names = match kind {
        EntityKind::Quest => &db.quest_names,
        EntityKind::Unit => &db.unit_names,
        EntityKind::Item => &db.item_names,
        EntityKind::Object => &db.object_names,
        _ => return None,
    };

    names
        .iter()
        .find(|(n, _)| n.to_lowercase() == name_lower)
        .or_else(|| {
            names
                .iter()
                .find(|(n, _)| n.to_lowercase().starts_with(&name_lower))
        })
        .map(|(n, id)| (n.clone(), *id))
}
