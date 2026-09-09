use std::collections::HashMap;
use std::path::Path;

use super::lua_parser::{self, LuaKey, LuaValue, ParsedFile};
use super::types::{Item, Object, Quest, Unit, Zone};

#[derive(Debug)]
pub struct Database {
    pub quests: HashMap<u32, Quest>,
    pub units: HashMap<u32, Unit>,
    pub items: HashMap<u32, Item>,
    pub objects: HashMap<u32, Object>,
    pub zones: HashMap<u32, Zone>,

    pub quest_names: Vec<(String, u32)>,
    pub unit_names: Vec<(String, u32)>,
    pub item_names: Vec<(String, u32)>,
    pub object_names: Vec<(String, u32)>,
    pub zone_names: Vec<(String, u32)>,
    pub zone_name_to_id: HashMap<String, u32>,
}

struct RawEntries {
    data: HashMap<u32, LuaValue>,
    locale: HashMap<u32, LuaValue>,
}

impl RawEntries {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
            locale: HashMap::new(),
        }
    }

    fn merge_parsed(&mut self, parsed: ParsedFile) {
        let variant = &parsed.header.variant;
        let is_locale = variant.starts_with("enUS");
        let target = if is_locale {
            &mut self.locale
        } else {
            &mut self.data
        };

        for (key, value) in parsed.entries {
            let LuaKey::Integer(id) = key else {
                continue;
            };
            let id = id as u32;

            if let LuaValue::String(ref s) = value {
                if s == "_" {
                    target.remove(&id);
                    continue;
                }
            }
            target.insert(id, value);
        }
    }
}

fn parse_file_at(path: &Path) -> Result<ParsedFile, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    lua_parser::parse_file(&content)
        .map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

fn parse_overwrites(path: &Path) -> Result<Vec<(String, u32)>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;

    let re = regex::Regex::new(
        r#"pfDB\["(\w+)"\]\["(?:data|enUS)-epoch"\]\[(\d+)\]\s*=\s*(?:\{\s*\}|"")"#,
    )
    .unwrap();

    let mut deletions = Vec::new();
    for cap in re.captures_iter(&content) {
        let category = cap[1].to_string();
        let id: u32 = cap[2].parse().unwrap_or(0);
        if id > 0 {
            deletions.push((category, id));
        }
    }
    Ok(deletions)
}

impl Database {
    pub fn load(pfquest_dir: &Path, pfquest_epoch_dir: &Path) -> Result<Self, String> {
        let mut quests = RawEntries::new();
        let mut units = RawEntries::new();
        let mut items = RawEntries::new();
        let mut objects = RawEntries::new();
        let mut zones = RawEntries::new();

        fn load_pair(
            entries: &mut RawEntries,
            base_dir: &Path,
            data_file: &str,
            locale_file: &str,
        ) -> Result<(), String> {
            let data_path = base_dir.join("db").join(data_file);
            let locale_path = base_dir.join("db").join(locale_file);
            if data_path.exists() {
                entries.merge_parsed(parse_file_at(&data_path)?);
            }
            if locale_path.exists() {
                entries.merge_parsed(parse_file_at(&locale_path)?);
            }
            Ok(())
        }

        load_pair(&mut quests, pfquest_dir, "quests.lua", "enUS/quests.lua")?;
        load_pair(&mut units, pfquest_dir, "units.lua", "enUS/units.lua")?;
        load_pair(&mut items, pfquest_dir, "items.lua", "enUS/items.lua")?;
        load_pair(&mut objects, pfquest_dir, "objects.lua", "enUS/objects.lua")?;
        load_pair(&mut zones, pfquest_dir, "zones.lua", "enUS/zones.lua")?;

        load_pair(
            &mut quests,
            pfquest_epoch_dir,
            "quests-epoch.lua",
            "enUS/quests-epoch.lua",
        )?;
        load_pair(
            &mut units,
            pfquest_epoch_dir,
            "units-epoch.lua",
            "enUS/units-epoch.lua",
        )?;
        load_pair(
            &mut items,
            pfquest_epoch_dir,
            "items-epoch.lua",
            "enUS/items-epoch.lua",
        )?;
        load_pair(
            &mut objects,
            pfquest_epoch_dir,
            "objects-epoch.lua",
            "enUS/objects-epoch.lua",
        )?;
        load_pair(
            &mut zones,
            pfquest_epoch_dir,
            "zones-epoch.lua",
            "enUS/zones-epoch.lua",
        )?;

        let overwrites_path = pfquest_epoch_dir.join("overwrites.lua");
        if overwrites_path.exists() {
            let deletions = parse_overwrites(&overwrites_path)?;
            for (category, id) in &deletions {
                match category.as_str() {
                    "quests" => {
                        quests.data.remove(id);
                        quests.locale.remove(id);
                    }
                    "units" => {
                        units.data.remove(id);
                        units.locale.remove(id);
                    }
                    "items" => {
                        items.data.remove(id);
                        items.locale.remove(id);
                    }
                    "objects" => {
                        objects.data.remove(id);
                        objects.locale.remove(id);
                    }
                    "zones" => {
                        zones.data.remove(id);
                        zones.locale.remove(id);
                    }
                    _ => {}
                }
            }
            log::info!("applied {} overwrites deletions", deletions.len());
        }

        let mut db = Database {
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
        };

        let all_quest_ids: std::collections::HashSet<u32> = quests
            .data
            .keys()
            .chain(quests.locale.keys())
            .copied()
            .collect();
        for id in all_quest_ids {
            let q = Quest::from_lua(id, quests.data.get(&id), quests.locale.get(&id));
            if !q.is_empty() {
                db.quests.insert(id, q);
            }
        }

        let all_unit_ids: std::collections::HashSet<u32> = units
            .data
            .keys()
            .chain(units.locale.keys())
            .copied()
            .collect();
        for id in all_unit_ids {
            let name = units
                .locale
                .get(&id)
                .and_then(|v| v.as_string())
                .unwrap_or("");
            let u = Unit::from_lua(id, units.data.get(&id), name);
            if !u.is_empty() {
                db.units.insert(id, u);
            }
        }

        let all_item_ids: std::collections::HashSet<u32> = items
            .data
            .keys()
            .chain(items.locale.keys())
            .copied()
            .collect();
        for id in all_item_ids {
            let name = items
                .locale
                .get(&id)
                .and_then(|v| v.as_string())
                .unwrap_or("");
            let i = Item::from_lua(id, items.data.get(&id), name);
            if !i.is_empty() {
                db.items.insert(id, i);
            }
        }

        let all_object_ids: std::collections::HashSet<u32> = objects
            .data
            .keys()
            .chain(objects.locale.keys())
            .copied()
            .collect();
        for id in all_object_ids {
            let name = objects
                .locale
                .get(&id)
                .and_then(|v| v.as_string())
                .unwrap_or("");
            let o = Object::from_lua(id, objects.data.get(&id), name);
            if !o.is_empty() {
                db.objects.insert(id, o);
            }
        }

        let all_zone_ids: std::collections::HashSet<u32> = zones
            .data
            .keys()
            .chain(zones.locale.keys())
            .copied()
            .collect();
        for id in all_zone_ids {
            let name = zones
                .locale
                .get(&id)
                .and_then(|v| v.as_string())
                .unwrap_or("");
            let z = Zone::from_lua(id, zones.data.get(&id), name);
            if !z.name.is_empty() {
                db.zones.insert(id, z);
            }
        }

        db.build_name_indexes();

        log::info!(
            "database loaded: {} quests, {} units, {} items, {} objects, {} zones",
            db.quests.len(),
            db.units.len(),
            db.items.len(),
            db.objects.len(),
            db.zones.len()
        );

        Ok(db)
    }

    fn build_name_indexes(&mut self) {
        self.quest_names = self
            .quests
            .iter()
            .filter(|(_, q)| !q.title.is_empty())
            .map(|(id, q)| (q.title.clone(), *id))
            .collect();
        self.quest_names.sort_by(|a, b| a.0.cmp(&b.0));

        self.unit_names = self
            .units
            .iter()
            .filter(|(_, u)| !u.name.is_empty())
            .map(|(id, u)| (u.name.clone(), *id))
            .collect();
        self.unit_names.sort_by(|a, b| a.0.cmp(&b.0));

        self.item_names = self
            .items
            .iter()
            .filter(|(_, i)| !i.name.is_empty())
            .map(|(id, i)| (i.name.clone(), *id))
            .collect();
        self.item_names.sort_by(|a, b| a.0.cmp(&b.0));

        self.object_names = self
            .objects
            .iter()
            .filter(|(_, o)| !o.name.is_empty())
            .map(|(id, o)| (o.name.clone(), *id))
            .collect();
        self.object_names.sort_by(|a, b| a.0.cmp(&b.0));

        self.zone_names = self
            .zones
            .iter()
            .filter(|(_, z)| !z.name.is_empty())
            .map(|(id, z)| (z.name.clone(), *id))
            .collect();
        self.zone_names.sort_by(|a, b| a.0.cmp(&b.0));

        self.zone_name_to_id = self
            .zones
            .iter()
            .filter(|(_, z)| !z.name.is_empty())
            .map(|(id, z)| (z.name.to_lowercase(), *id))
            .collect();
    }

    pub fn lookup_quest(&self, id: u32) -> Option<&Quest> {
        self.quests.get(&id)
    }

    pub fn lookup_unit(&self, id: u32) -> Option<&Unit> {
        self.units.get(&id)
    }

    pub fn lookup_item(&self, id: u32) -> Option<&Item> {
        self.items.get(&id)
    }

    pub fn lookup_object(&self, id: u32) -> Option<&Object> {
        self.objects.get(&id)
    }

    pub fn lookup_zone_by_name(&self, name: &str) -> Option<&Zone> {
        let id = self.zone_name_to_id.get(&name.to_lowercase())?;
        self.zones.get(id)
    }

    pub fn entity_exists(&self, id: u32) -> bool {
        self.quests.contains_key(&id)
            || self.units.contains_key(&id)
            || self.items.contains_key(&id)
            || self.objects.contains_key(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    // #[ignore]
    fn test_load_real_database() {
        let pf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../pfQuest");
        let epoch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../pfQuest-epoch");

        if !pf.exists() || !epoch.exists() {
            panic!("pfQuest directories not found");
        }

        let db = Database::load(&pf, &epoch).expect("failed to load database");

        assert!(
            db.quests.len() > 5000,
            "expected >5000 quests, got {}",
            db.quests.len()
        );
        assert!(
            db.units.len() > 10000,
            "expected >10000 units, got {}",
            db.units.len()
        );
        assert!(
            db.items.len() > 15000,
            "expected >15000 items, got {}",
            db.items.len()
        );
        assert!(
            db.objects.len() > 5000,
            "expected >5000 objects, got {}",
            db.objects.len()
        );
        assert!(
            db.zones.len() > 1000,
            "expected >1000 zones, got {}",
            db.zones.len()
        );

        // Verify a known quest exists
        let q = db.lookup_quest(783).expect("quest 783 should exist");
        assert_eq!(q.title, "A Threat Within");

        // Verify overwrites removed Silithus NPCs
        assert!(
            db.lookup_unit(15169).is_none(),
            "unit 15169 should be removed by overwrites"
        );

        // Verify zone lookup
        let z = db.lookup_zone_by_name("Elwynn Forest");
        assert!(z.is_some(), "Elwynn Forest should exist");

        println!("Database loaded successfully:");
        println!("  Quests: {}", db.quests.len());
        println!("  Units: {}", db.units.len());
        println!("  Items: {}", db.items.len());
        println!("  Objects: {}", db.objects.len());
        println!("  Zones: {}", db.zones.len());
    }
}
