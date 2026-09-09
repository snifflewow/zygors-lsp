use super::lua_parser::LuaValue;

#[derive(Debug, Clone)]
pub struct Quest {
    pub id: u32,
    pub title: String,
    pub objective_text: String,
    pub description: String,
    pub level: i32,
    pub min_level: i32,
    pub start_units: Vec<u32>,
    pub start_objects: Vec<u32>,
    pub start_items: Vec<u32>,
    pub end_units: Vec<u32>,
    pub end_objects: Vec<u32>,
    pub obj_units: Vec<u32>,
    pub obj_items: Vec<u32>,
    pub obj_objects: Vec<u32>,
    pub pre: Vec<u32>,
    pub next: Vec<u32>,
    pub close: Vec<u32>,
    pub race: u32,
    pub class: u32,
}

#[derive(Debug, Clone)]
pub struct Coord {
    pub x: f64,
    pub y: f64,
    pub zone_id: u32,
}

#[derive(Debug, Clone)]
pub struct Unit {
    pub id: u32,
    pub name: String,
    pub coords: Vec<Coord>,
    pub faction: Option<String>,
    pub level: String,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: u32,
    pub name: String,
    pub drop_units: Vec<(u32, f64)>,
    pub drop_objects: Vec<(u32, f64)>,
    pub vendors: Vec<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub id: u32,
    pub name: String,
    pub coords: Vec<Coord>,
    pub faction: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub id: u32,
    pub name: String,
    pub parent_map: u32,
    pub width: f64,
    pub height: f64,
    pub x_offset: f64,
    pub y_offset: f64,
}

fn extract_sub_ids(data: &LuaValue, field: &str, sub: &str) -> Vec<u32> {
    data.get_field(field)
        .and_then(|f| f.get_field(sub))
        .map(|v| v.as_int_array())
        .unwrap_or_default()
}

fn extract_next(data: &LuaValue) -> Vec<u32> {
    match data.get_field("next") {
        Some(LuaValue::Number(n)) => vec![*n as u32],
        Some(LuaValue::Table(_)) => data.get_field("next").unwrap().as_int_array(),
        _ => vec![],
    }
}

fn parse_coords(data: &LuaValue) -> Vec<Coord> {
    let Some(coords_val) = data.get_field("coords") else {
        return vec![];
    };
    let Some(entries) = coords_val.get_int_keys() else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|(_, v)| {
            let t = v.as_table()?;
            let nums: Vec<f64> = t.iter().filter_map(|(_, v)| v.as_number()).collect();
            if nums.len() >= 3 {
                Some(Coord {
                    x: nums[0],
                    y: nums[1],
                    zone_id: nums[2] as u32,
                })
            } else {
                None
            }
        })
        .collect()
}

fn parse_id_rate_map(data: &LuaValue, field: &str) -> Vec<(u32, f64)> {
    let Some(map) = data.get_field(field) else {
        return vec![];
    };
    let Some(entries) = map.get_int_keys() else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|(id, v)| Some((*id as u32, v.as_number()?)))
        .collect()
}

fn parse_id_count_map(data: &LuaValue, field: &str) -> Vec<(u32, u32)> {
    let Some(map) = data.get_field(field) else {
        return vec![];
    };
    let Some(entries) = map.get_int_keys() else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|(id, v)| Some((*id as u32, v.as_number()? as u32)))
        .collect()
}

impl Quest {
    pub fn from_lua(id: u32, data: Option<&LuaValue>, locale: Option<&LuaValue>) -> Self {
        let (title, objective_text, description) = match locale {
            Some(LuaValue::Table(_)) => (
                locale
                    .and_then(|l| l.get_field("T"))
                    .and_then(|v| v.as_string())
                    .unwrap_or("")
                    .to_string(),
                locale
                    .and_then(|l| l.get_field("O"))
                    .and_then(|v| v.as_string())
                    .unwrap_or("")
                    .to_string(),
                locale
                    .and_then(|l| l.get_field("D"))
                    .and_then(|v| v.as_string())
                    .unwrap_or("")
                    .to_string(),
            ),
            Some(LuaValue::String(s)) => (s.clone(), String::new(), String::new()),
            _ => (String::new(), String::new(), String::new()),
        };

        let empty = LuaValue::Table(vec![]);
        let d = data.unwrap_or(&empty);

        Quest {
            id,
            title,
            objective_text,
            description,
            level: d.get_field("lvl").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
            min_level: d.get_field("min").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
            start_units: extract_sub_ids(d, "start", "U"),
            start_objects: extract_sub_ids(d, "start", "O"),
            start_items: extract_sub_ids(d, "start", "I"),
            end_units: extract_sub_ids(d, "end", "U"),
            end_objects: extract_sub_ids(d, "end", "O"),
            obj_units: extract_sub_ids(d, "obj", "U"),
            obj_items: extract_sub_ids(d, "obj", "I"),
            obj_objects: extract_sub_ids(d, "obj", "O"),
            pre: d
                .get_field("pre")
                .map(|v| v.as_int_array())
                .unwrap_or_default(),
            next: extract_next(d),
            close: d
                .get_field("close")
                .map(|v| v.as_int_array())
                .unwrap_or_default(),
            race: d
                .get_field("race")
                .and_then(|v| v.as_number())
                .unwrap_or(0.0) as u32,
            class: d
                .get_field("class")
                .and_then(|v| v.as_number())
                .unwrap_or(0.0) as u32,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.title.is_empty()
            && self.level == 0
            && self.start_units.is_empty()
            && self.end_units.is_empty()
    }
}

impl Unit {
    pub fn from_lua(id: u32, data: Option<&LuaValue>, name: &str) -> Self {
        let empty = LuaValue::Table(vec![]);
        let d = data.unwrap_or(&empty);

        Unit {
            id,
            name: name.to_string(),
            coords: parse_coords(d),
            faction: d.get_field("fac").and_then(|v| v.as_string()).map(|s| s.to_string()),
            level: d
                .get_field("lvl")
                .and_then(|v| v.as_string())
                .unwrap_or("")
                .to_string(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.coords.is_empty()
    }
}

impl Item {
    pub fn from_lua(id: u32, data: Option<&LuaValue>, name: &str) -> Self {
        let empty = LuaValue::Table(vec![]);
        let d = data.unwrap_or(&empty);

        Item {
            id,
            name: name.to_string(),
            drop_units: parse_id_rate_map(d, "U"),
            drop_objects: parse_id_rate_map(d, "O"),
            vendors: parse_id_count_map(d, "V"),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
            && self.drop_units.is_empty()
            && self.drop_objects.is_empty()
            && self.vendors.is_empty()
    }
}

impl Object {
    pub fn from_lua(id: u32, data: Option<&LuaValue>, name: &str) -> Self {
        let empty = LuaValue::Table(vec![]);
        let d = data.unwrap_or(&empty);

        Object {
            id,
            name: name.to_string(),
            coords: parse_coords(d),
            faction: d.get_field("fac").and_then(|v| v.as_string()).map(|s| s.to_string()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.coords.is_empty()
    }
}

impl Zone {
    pub fn from_lua(id: u32, data: Option<&LuaValue>, name: &str) -> Self {
        let nums: Vec<f64> = data
            .and_then(|d| d.as_table())
            .map(|t| t.iter().filter_map(|(_, v)| v.as_number()).collect())
            .unwrap_or_default();

        Zone {
            id,
            name: name.to_string(),
            parent_map: nums.first().copied().unwrap_or(0.0) as u32,
            width: nums.get(1).copied().unwrap_or(0.0),
            height: nums.get(2).copied().unwrap_or(0.0),
            x_offset: nums.get(3).copied().unwrap_or(0.0),
            y_offset: nums.get(4).copied().unwrap_or(0.0),
        }
    }
}
