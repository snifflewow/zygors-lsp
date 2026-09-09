use super::parser::GuideRegion;
use super::types::*;

pub fn build_model(region: &GuideRegion) -> GuideModel {
    let mut model = GuideModel::new(region.title.clone());
    model.next_guide = region.next_guide.clone();

    let Some(tree) = &region.tree else {
        return model;
    };

    let root = tree.root_node();
    let source = region.content.as_bytes();

    let mut current_step_start: Option<usize> = None;
    let mut open_stickies: Vec<String> = Vec::new();
    let mut step_index: usize = 0;

    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        match node.kind() {
            "step_line" => {
                if let Some(start) = current_step_start {
                    let step = Step {
                        byte_range: start..node.start_byte(),
                        sticky_labels: open_stickies.clone(),
                        only_condition: None,
                    };
                    model.steps.push(step);
                    step_index += 1;
                }
                current_step_start = Some(node.start_byte());
            }
            "action_line" | "text_line" | "quote_pipe_line" => {
                process_action_node(&node, source, &mut model, &mut open_stickies, step_index);
            }
            "only_line" => {
                if let Some(seg) = node.child_by_field_name("segment") {
                    let text = node_text(seg, source);
                    if let Some(step) = model.steps.last_mut() {
                        step.only_condition = Some(text);
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(start) = current_step_start {
        let step = Step {
            byte_range: start..root.end_byte(),
            sticky_labels: open_stickies.clone(),
            only_condition: None,
        };
        model.steps.push(step);
    }

    model
}

fn process_action_node(
    node: &tree_sitter::Node,
    source: &[u8],
    model: &mut GuideModel,
    open_stickies: &mut Vec<String>,
    step_index: usize,
) {
    let action = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "action_keyword")
        .map(|c| node_text(c, source));

    let action_str = action.as_deref().unwrap_or("");

    match action_str {
        "stickystart" => {
            if let Some(label) = extract_quoted_string(node, source) {
                model.sticky_starts.insert(label.clone(), step_index);
                open_stickies.push(label.clone());
                model.label_refs.push(LabelRef {
                    name: label,
                    byte_range: node.start_byte()..node.end_byte(),
                });
            }
        }
        "stickystop" => {
            if let Some(label) = extract_quoted_string(node, source) {
                model.sticky_stops.insert(label.clone(), step_index);
                open_stickies.retain(|l| l != &label);
            }
        }
        "label" => {
            if let Some(label) = extract_quoted_string(node, source) {
                model
                    .labels
                    .entry(label.clone())
                    .or_default()
                    .push(step_index);
                open_stickies.retain(|l| l != &label);
                model.label_refs.push(LabelRef {
                    name: label,
                    byte_range: node.start_byte()..node.end_byte(),
                });
            }
        }
        _ => {}
    }

    let mut child_cursor = node.walk();
    for child in node.children(&mut child_cursor) {
        match child.kind() {
            "name_id" => {
                let text = node_text(child, source);
                if let Some((name, id)) = parse_name_id(&text) {
                    model.entity_refs.push(EntityRef {
                        name,
                        id,
                        kind: EntityKind::from_action(action_str),
                        byte_range: child.start_byte()..child.end_byte(),
                        action: action_str.to_string(),
                    });
                }
            }
            "segment" => {
                extract_refs_from_segment(&child, source, action_str, model);
            }
            "pipe_directive" => {
                process_pipe_directive(&child, source, model);
            }
            _ => {}
        }
    }
}

fn extract_refs_from_segment(
    node: &tree_sitter::Node,
    source: &[u8],
    action: &str,
    model: &mut GuideModel,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name_id" {
            let text = node_text(child, source);
            if let Some((name, id)) = parse_name_id(&text) {
                model.entity_refs.push(EntityRef {
                    name,
                    id,
                    kind: EntityKind::from_action(action),
                    byte_range: child.start_byte()..child.end_byte(),
                    action: action.to_string(),
                });
            }
        }
    }
}

fn process_pipe_directive(
    node: &tree_sitter::Node,
    source: &[u8],
    model: &mut GuideModel,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "pipe_quest" => {
                let nums: Vec<u32> = child
                    .children(&mut child.walk())
                    .filter(|c| c.kind() == "number")
                    .filter_map(|c| node_text(c, source).parse().ok())
                    .collect();
                if let Some(&quest_id) = nums.first() {
                    model.quest_refs.push(QuestRef {
                        quest_id,
                        objective: nums.get(1).copied(),
                        byte_range: child.start_byte()..child.end_byte(),
                    });
                }
            }
            "pipe_goto" => {
                if let Some(seg) = child.children(&mut child.walk()).find(|c| c.kind() == "segment") {
                    let goto = parse_goto_segment(&seg, source);
                    model.goto_refs.push(GotoRef {
                        zone: goto.0,
                        x: goto.1,
                        y: goto.2,
                        byte_range: child.start_byte()..child.end_byte(),
                    });
                }
            }
            "pipe_with_value" => {
                let directive = child
                    .children(&mut child.walk())
                    .find(|c| c.kind() == "word")
                    .map(|c| node_text(c, source));

                if let Some(ref d) = directive {
                    let pipe_action = match d.as_str() {
                        "vendor" | "talk" => "talk",
                        "havebuff" | "nobuff" => "havebuff",
                        "learnpetspell" => "learnpetspell",
                        _ => "",
                    };
                    if !pipe_action.is_empty() {
                        if let Some(seg) = child.children(&mut child.walk()).find(|c| c.kind() == "segment") {
                            extract_refs_from_segment(&seg, source, pipe_action, model);
                        }
                    }

                    if d == "next" {
                        if let Some(seg) = child.children(&mut child.walk()).find(|c| c.kind() == "segment") {
                            if let Some(qs) = extract_quoted_from_segment(&seg, source) {
                                model.label_refs.push(LabelRef {
                                    name: qs,
                                    byte_range: child.start_byte()..child.end_byte(),
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                if child.kind() == "segment" {
                    let mut seg_cursor = child.walk();
                    for seg_child in child.children(&mut seg_cursor) {
                        if seg_child.kind() == "name_id" {
                            let text = node_text(seg_child, source);
                            if let Some((name, id)) = parse_name_id(&text) {
                                model.entity_refs.push(EntityRef {
                                    name,
                                    id,
                                    kind: EntityKind::Unknown,
                                    byte_range: seg_child.start_byte()..seg_child.end_byte(),
                                    action: String::new(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn parse_goto_segment(
    node: &tree_sitter::Node,
    source: &[u8],
) -> (Option<String>, Option<f64>, Option<f64>) {
    let mut zone_words = Vec::new();
    let mut coord: Option<(f64, f64)> = None;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "coordinate" => {
                let text = node_text(child, source);
                let parts: Vec<&str> = text.split(',').collect();
                if parts.len() >= 2 {
                    let x = parts[0].parse().ok();
                    let y = parts[1].parse().ok();
                    if let (Some(x), Some(y)) = (x, y) {
                        coord = Some((x, y));
                    }
                }
            }
            "word" => {
                zone_words.push(node_text(child, source));
            }
            _ => {}
        }
    }

    let zone = if zone_words.is_empty() {
        None
    } else {
        Some(zone_words.join(" "))
    };

    (zone, coord.map(|c| c.0), coord.map(|c| c.1))
}

fn parse_name_id(text: &str) -> Option<(String, u32)> {
    let sep = text.find("##")?;
    let name = text[..sep].to_string();
    let id_str = &text[sep + 2..];
    let id_str = id_str.split('/').next()?;
    let id_str = id_str.trim_end_matches('+');
    let id: u32 = id_str.parse().ok()?;
    Some((name, id))
}

fn extract_quoted_string(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "quoted_string" {
            let text = node_text(child, source);
            let trimmed = text.trim_matches('"');
            return Some(trimmed.to_string());
        }
        if child.kind() == "segment" {
            return extract_quoted_from_segment(&child, source);
        }
    }
    None
}

fn extract_quoted_from_segment(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "quoted_string" {
            let text = node_text(child, source);
            let trimmed = text.trim_matches('"');
            return Some(trimmed.to_string());
        }
    }
    None
}

fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()])
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guide::parser::DocumentParser;

    fn parse_guide(content: &str) -> GuideModel {
        let source = format!(
            "ZygorGuidesViewer:RegisterGuide('Test', {{}}, [[{}]])",
            content
        );
        let mut parser = DocumentParser::new();
        let regions = parser.parse_document(&source);
        assert_eq!(regions.len(), 1);
        build_model(&regions[0])
    }

    #[test]
    fn test_steps() {
        let model = parse_guide(
            r#"
step
accept A Threat Within##783
step
turnin Deputy Willem##823
"#,
        );
        assert_eq!(model.steps.len(), 2);
    }

    #[test]
    fn test_entity_refs() {
        let model = parse_guide(
            r#"
step
accept A Threat Within##783
talk Deputy Willem##823
collect Tough Wolf Meat##750
"#,
        );
        assert_eq!(model.entity_refs.len(), 3);
        assert_eq!(model.entity_refs[0].name, "A Threat Within");
        assert_eq!(model.entity_refs[0].id, 783);
        assert_eq!(model.entity_refs[0].kind, EntityKind::Quest);
        assert_eq!(model.entity_refs[1].kind, EntityKind::Unit);
        assert_eq!(model.entity_refs[2].kind, EntityKind::Item);
    }

    #[test]
    fn test_quest_refs() {
        let model = parse_guide(
            r#"
step
accept A Threat Within##783 |q 783/1
"#,
        );
        assert_eq!(model.quest_refs.len(), 1);
        assert_eq!(model.quest_refs[0].quest_id, 783);
        assert_eq!(model.quest_refs[0].objective, Some(1));
    }

    #[test]
    fn test_sticky_labels() {
        let model = parse_guide(
            r#"
step
stickystart "Collect_Gold"
kill Kobold##475
step
label "Collect_Gold"
collect 10 Gold Dust##773
"#,
        );
        assert!(model.sticky_starts.contains_key("Collect_Gold"));
        assert!(model.labels.contains_key("Collect_Gold"));
        assert_eq!(model.label_refs.len(), 2);
    }

    #[test]
    fn test_parse_name_id() {
        assert_eq!(
            parse_name_id("A Threat Within##783"),
            Some(("A Threat Within".to_string(), 783))
        );
        assert_eq!(
            parse_name_id("Quest##123/1"),
            Some(("Quest".to_string(), 123))
        );
        assert_eq!(
            parse_name_id("Mob##456+"),
            Some(("Mob".to_string(), 456))
        );
        assert_eq!(parse_name_id("no separator"), None);
    }
}
