use tower_lsp::lsp_types::*;

use crate::guide::types::*;

pub fn position_to_byte_offset(source: &str, pos: Position) -> usize {
    let mut line = 0u32;
    for (i, ch) in source.char_indices() {
        if line == pos.line {
            let line_start = i;
            return line_start + pos.character as usize;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    source.len()
}

pub fn byte_offset_to_position(source: &str, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.matches('\n').count() as u32;
    let last_newline = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = (offset - last_newline) as u32;
    Position { line, character }
}

pub fn byte_range_to_lsp(
    source: &str,
    content_offset: usize,
    range: &std::ops::Range<usize>,
) -> Range {
    let start = byte_offset_to_position(source, content_offset + range.start);
    let end = byte_offset_to_position(source, content_offset + range.end);
    Range { start, end }
}

pub enum HitResult<'a> {
    Entity(&'a EntityRef),
    Quest(&'a QuestRef),
    Goto(&'a GotoRef),
    Label(&'a LabelRef),
    Nothing,
}

pub fn find_ref_at_offset<'a>(model: &'a GuideModel, offset: usize) -> HitResult<'a> {
    for r in &model.entity_refs {
        if r.byte_range.contains(&offset) {
            return HitResult::Entity(r);
        }
    }
    for r in &model.quest_refs {
        if r.byte_range.contains(&offset) {
            return HitResult::Quest(r);
        }
    }
    for r in &model.goto_refs {
        if r.byte_range.contains(&offset) {
            return HitResult::Goto(r);
        }
    }
    for r in &model.label_refs {
        if r.byte_range.contains(&offset) {
            return HitResult::Label(r);
        }
    }
    HitResult::Nothing
}

pub fn get_line_up_to_cursor(source: &str, pos: Position) -> Option<String> {
    let mut line = 0u32;
    let mut line_start = 0;
    for (i, ch) in source.char_indices() {
        if line == pos.line {
            line_start = i;
            let end = i + pos.character as usize;
            let end = end.min(source.len());
            return Some(source[line_start..end].to_string());
        }
        if ch == '\n' {
            line += 1;
        }
    }
    if line == pos.line {
        let end = (line_start + pos.character as usize).min(source.len());
        return Some(source[line_start..end].to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_roundtrip() {
        let source = "line one\nline two\nline three";
        let pos = Position { line: 1, character: 5 };
        let offset = position_to_byte_offset(source, pos);
        let back = byte_offset_to_position(source, offset);
        assert_eq!(back, pos);
    }

    #[test]
    fn test_get_line_up_to_cursor() {
        let source = "step\naccept A Threat##783\nturnin Foo##1";
        let line = get_line_up_to_cursor(source, Position { line: 1, character: 7 });
        assert_eq!(line.as_deref(), Some("accept "));
    }
}
