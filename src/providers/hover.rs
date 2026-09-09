use tower_lsp::lsp_types::*;

use super::position::HitResult;
use crate::db::index::Database;
use crate::db::types;
use crate::guide::types::*;

pub fn hover_info(hit: &HitResult, db: &Database) -> Option<Hover> {
    let markdown = match hit {
        HitResult::Entity(eref) => hover_entity(eref, db)?,
        HitResult::Quest(qref) => hover_quest_pipe(qref, db)?,
        HitResult::Goto(gref) => hover_goto(gref, db)?,
        HitResult::Label(lref) => format!("**Label:** `{}`", lref.name),
        HitResult::Nothing => return None,
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: None,
    })
}

fn hover_entity(eref: &EntityRef, db: &Database) -> Option<String> {
    let id = eref.id;

    match eref.kind {
        EntityKind::Quest => {
            if let Some(q) = db.quests.get(&id) {
                return Some(format_quest(q, id, db));
            }
        }
        EntityKind::Unit => {
            if let Some(u) = db.units.get(&id) {
                return Some(format_unit(u, id, db));
            }
        }
        EntityKind::Item => {
            if let Some(i) = db.items.get(&id) {
                return Some(format_item(i, id, db));
            }
        }
        EntityKind::Object => {
            if let Some(o) = db.objects.get(&id) {
                return Some(format_object(o, id, db));
            }
        }
        _ => {}
    }

    if let Some(q) = db.quests.get(&id) {
        return Some(format_quest(q, id, db));
    }
    if let Some(u) = db.units.get(&id) {
        return Some(format_unit(u, id, db));
    }
    if let Some(i) = db.items.get(&id) {
        return Some(format_item(i, id, db));
    }
    if let Some(o) = db.objects.get(&id) {
        return Some(format_object(o, id, db));
    }
    None
}

fn format_quest(q: &types::Quest, id: u32, db: &Database) -> String {
    let mut lines = vec![format!("**Quest:** {} `[{}]`", q.title, id)];
    if q.level > 0 {
        lines.push(format!("**Level:** {} (min {})", q.level, q.min_level));
    }
    if !q.objective_text.is_empty() {
        let obj = q.objective_text.replace("$B$B", "\n").replace("$B", "\n");
        lines.push(format!("**Objectives:** {}", obj.trim()));
    }
    if !q.start_units.is_empty() {
        let names: Vec<String> = q
            .start_units
            .iter()
            .filter_map(|uid| db.units.get(uid).map(|u| u.name.clone()))
            .collect();
        if !names.is_empty() {
            lines.push(format!("**Start:** {}", names.join(", ")));
        }
    }
    if !q.end_units.is_empty() {
        let names: Vec<String> = q
            .end_units
            .iter()
            .filter_map(|uid| db.units.get(uid).map(|u| u.name.clone()))
            .collect();
        if !names.is_empty() {
            lines.push(format!("**Turn in:** {}", names.join(", ")));
        }
    }
    if !q.pre.is_empty() {
        let names: Vec<String> = q
            .pre
            .iter()
            .filter_map(|qid| db.quests.get(qid).map(|pq| pq.title.clone()))
            .collect();
        if !names.is_empty() {
            lines.push(format!("**Requires:** {}", names.join(", ")));
        }
    }
    lines.join("\n\n")
}

fn format_unit(u: &types::Unit, id: u32, db: &Database) -> String {
    let mut lines = vec![format!("**NPC:** {} `[{}]`", u.name, id)];
    if !u.level.is_empty() {
        lines.push(format!("**Level:** {}", u.level));
    }
    if let Some(fac) = &u.faction {
        let faction = match fac.as_str() {
            "A" => "Alliance",
            "H" => "Horde",
            "AH" => "Both",
            _ => fac,
        };
        lines.push(format!("**Faction:** {}", faction));
    }
    if !u.coords.is_empty() {
        let zones: Vec<String> = u
            .coords
            .iter()
            .filter_map(|c| {
                db.zones
                    .get(&c.zone_id)
                    .map(|z| format!("{} ({:.1}, {:.1})", z.name, c.x, c.y))
            })
            .take(3)
            .collect();
        if !zones.is_empty() {
            lines.push(format!("**Location:** {}", zones.join("; ")));
        }
    }
    lines.join("\n\n")
}

fn format_item(i: &types::Item, id: u32, db: &Database) -> String {
    let mut lines = vec![format!("**Item:** {} `[{}]`", i.name, id)];
    if !i.drop_units.is_empty() {
        let drops: Vec<String> = i
            .drop_units
            .iter()
            .take(5)
            .filter_map(|(uid, rate)| {
                db.units
                    .get(uid)
                    .map(|u| format!("{} ({:.1}%)", u.name, rate))
            })
            .collect();
        if !drops.is_empty() {
            lines.push(format!("**Drops from:** {}", drops.join(", ")));
        }
    }
    if !i.vendors.is_empty() {
        let vendors: Vec<String> = i
            .vendors
            .iter()
            .take(3)
            .filter_map(|(uid, _)| db.units.get(uid).map(|u| u.name.clone()))
            .collect();
        if !vendors.is_empty() {
            lines.push(format!("**Sold by:** {}", vendors.join(", ")));
        }
    }
    lines.join("\n\n")
}

fn format_object(o: &types::Object, id: u32, db: &Database) -> String {
    let mut lines = vec![format!("**Object:** {} `[{}]`", o.name, id)];
    if !o.coords.is_empty() {
        let zones: Vec<String> = o
            .coords
            .iter()
            .filter_map(|c| {
                db.zones
                    .get(&c.zone_id)
                    .map(|z| format!("{} ({:.1}, {:.1})", z.name, c.x, c.y))
            })
            .take(3)
            .collect();
        if !zones.is_empty() {
            lines.push(format!("**Location:** {}", zones.join("; ")));
        }
    }
    lines.join("\n\n")
}

fn hover_quest_pipe(qref: &QuestRef, db: &Database) -> Option<String> {
    let q = db.quests.get(&qref.quest_id)?;
    let mut text = format!("**Quest:** {} `[{}]`", q.title, qref.quest_id);
    if let Some(obj) = qref.objective {
        text.push_str(&format!(" — objective {}", obj));
    }
    if q.level > 0 {
        text.push_str(&format!("\n\n**Level:** {}", q.level));
    }
    Some(text)
}

fn hover_goto(gref: &GotoRef, db: &Database) -> Option<String> {
    let zone_name = gref.zone.as_ref()?;
    let zone = db.lookup_zone_by_name(zone_name)?;
    let mut text = format!("**Zone:** {} `[{}]`", zone.name, zone.id);
    if let (Some(x), Some(y)) = (gref.x, gref.y) {
        text.push_str(&format!("\n\n**Coordinates:** ({:.1}, {:.1})", x, y));
    }
    Some(text)
}
