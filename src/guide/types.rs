use std::collections::HashMap;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Quest,
    Unit,
    Item,
    Object,
    Spell,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct EntityRef {
    pub name: String,
    pub id: u32,
    pub kind: EntityKind,
    pub byte_range: Range<usize>,
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct QuestRef {
    pub quest_id: u32,
    pub objective: Option<u32>,
    pub byte_range: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct GotoRef {
    pub zone: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub byte_range: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct LabelRef {
    pub name: String,
    pub byte_range: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub byte_range: Range<usize>,
    pub sticky_labels: Vec<String>,
    pub only_condition: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GuideModel {
    pub title: String,
    pub next_guide: Option<String>,
    pub steps: Vec<Step>,
    pub labels: HashMap<String, Vec<usize>>,
    pub sticky_starts: HashMap<String, usize>,
    pub sticky_stops: HashMap<String, usize>,
    pub entity_refs: Vec<EntityRef>,
    pub quest_refs: Vec<QuestRef>,
    pub goto_refs: Vec<GotoRef>,
    pub label_refs: Vec<LabelRef>,
}

impl GuideModel {
    pub fn new(title: String) -> Self {
        Self {
            title,
            next_guide: None,
            steps: Vec::new(),
            labels: HashMap::new(),
            sticky_starts: HashMap::new(),
            sticky_stops: HashMap::new(),
            entity_refs: Vec::new(),
            quest_refs: Vec::new(),
            goto_refs: Vec::new(),
            label_refs: Vec::new(),
        }
    }
}

impl EntityKind {
    pub fn from_action(action: &str) -> Self {
        match action {
            "accept" | "turnin" => EntityKind::Quest,
            "talk" | "kill" | "clicknpc" | "from" => EntityKind::Unit,
            "collect" | "use" | "buy" | "trash" | "destroy" | "bank" | "equip" | "equipped"
            | "mine" => EntityKind::Item,
            "click" => EntityKind::Object,
            "learnspell" | "learnpetspell" | "learn" | "cast" | "havebuff" | "nobuff" => {
                EntityKind::Spell
            }
            "get" | "goal" => EntityKind::Unknown,
            _ => EntityKind::Unknown,
        }
    }
}
