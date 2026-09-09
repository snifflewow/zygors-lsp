use std::ops::Range;

use tree_sitter::Parser;

#[derive(Debug)]
pub struct GuideRegion {
    pub title: String,
    pub next_guide: Option<String>,
    pub content: String,
    pub content_offset: usize,
    pub tree: Option<tree_sitter::Tree>,
}

pub struct DocumentParser {
    ts_parser: Parser,
}

impl DocumentParser {
    pub fn new() -> Self {
        let mut ts_parser = Parser::new();
        ts_parser
            .set_language(&tree_sitter_zygorguide::LANGUAGE.into())
            .expect("failed to load zygorguide grammar");

        Self { ts_parser }
    }

    pub fn parse_document(&mut self, source: &str) -> Vec<GuideRegion> {
        let regions = extract_guide_regions(source);
        regions
            .into_iter()
            .map(|mut region| {
                region.tree = self.ts_parser.parse(&region.content, None);
                region
            })
            .collect()
    }

    pub fn reparse(&mut self, content: &str, old_tree: Option<&tree_sitter::Tree>) -> Option<tree_sitter::Tree> {
        self.ts_parser.parse(content, old_tree)
    }
}

fn extract_guide_regions(source: &str) -> Vec<GuideRegion> {
    let mut regions = Vec::new();
    let bytes = source.as_bytes();
    let mut search_from = 0;

    while search_from < bytes.len() {
        let Some(reg_pos) = find_register_guide(source, search_from) else {
            break;
        };

        let title = extract_title(source, reg_pos);

        let next_guide = extract_next_option(source, reg_pos);

        let Some((content_start, content_end)) = find_long_string(source, reg_pos) else {
            search_from = reg_pos + 1;
            continue;
        };

        let content = source[content_start..content_end].to_string();

        regions.push(GuideRegion {
            title,
            next_guide,
            content,
            content_offset: content_start,
            tree: None,
        });

        search_from = content_end + 2;
    }

    regions
}

fn find_register_guide(source: &str, from: usize) -> Option<usize> {
    source[from..].find("RegisterGuide(").map(|i| from + i)
}

fn extract_title(source: &str, reg_pos: usize) -> String {
    extract_title_inner(source, reg_pos).unwrap_or_default()
}

fn extract_title_inner(source: &str, reg_pos: usize) -> Option<String> {
    let after = &source[reg_pos..];
    let paren = after.find('(')?;
    let rest = &after[paren + 1..];
    let rest = rest.trim_start();

    if rest.starts_with('\'') {
        let end = rest[1..].find('\'')?;
        return Some(rest[1..1 + end].to_string());
    }
    if rest.starts_with('"') {
        let end = rest[1..].find('"')?;
        return Some(rest[1..1 + end].to_string());
    }
    None
}

fn extract_next_option(source: &str, reg_pos: usize) -> Option<String> {
    let search_end = std::cmp::min(reg_pos + 2000, source.len());
    let region = &source[reg_pos..search_end];

    let ll_pos = region.find("[[")?;
    let before_content = &region[..ll_pos];

    let next_pos = before_content.find("next")?;
    let after_next = &before_content[next_pos + 4..];
    let after_next = after_next.trim_start();

    if !after_next.starts_with('=') {
        return None;
    }
    let after_eq = after_next[1..].trim_start();

    if after_eq.starts_with('\'') {
        let end = after_eq[1..].find('\'')?;
        return Some(after_eq[1..1 + end].to_string());
    }
    if after_eq.starts_with('"') {
        let end = after_eq[1..].find('"')?;
        return Some(after_eq[1..1 + end].to_string());
    }
    None
}

fn find_long_string(source: &str, from: usize) -> Option<(usize, usize)> {
    let rest = &source[from..];
    let open = rest.find("[[")?;
    let content_start = from + open + 2;

    let after_open = &source[content_start..];
    let close = after_open.find("]]")?;
    let content_end = content_start + close;

    Some((content_start, content_end))
}

impl GuideRegion {
    pub fn byte_range(&self) -> Range<usize> {
        self.content_offset..self.content_offset + self.content.len()
    }

    pub fn offset_to_file_position(&self, offset: usize) -> usize {
        self.content_offset + offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_guide() {
        let source = r#"local ZygorGuidesViewer = ZygorGuidesViewer
if not ZygorGuidesViewer then return end
ZygorGuidesViewer:RegisterGuide(
  'Epoch\\Test Guide',
  { next = 'Epoch\\Next Guide' },
  [[
step
accept A Threat Within##783
|goto Elwynn Forest 48.2,42.8
]])
"#;
        let regions = extract_guide_regions(source);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].title, "Epoch\\\\Test Guide");
        assert_eq!(
            regions[0].next_guide.as_deref(),
            Some("Epoch\\\\Next Guide")
        );
        assert!(regions[0].content.contains("step"));
        assert!(regions[0].content.contains("accept"));
    }

    #[test]
    fn test_extract_multiple_guides() {
        let source = r#"local ZygorGuidesViewer = ZygorGuidesViewer
if not ZygorGuidesViewer then return end
ZygorGuidesViewer:RegisterGuide(
  'Guide One',
  {},
  [[
step
accept Quest##1
]]
)
ZygorGuidesViewer:RegisterGuide(
  'Guide Two',
  {},
  [[
step
accept Quest##2
]]
)
"#;
        let regions = extract_guide_regions(source);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].title, "Guide One");
        assert_eq!(regions[1].title, "Guide Two");
    }

    #[test]
    fn test_parse_with_treesitter() {
        let mut parser = DocumentParser::new();
        let source = r#"ZygorGuidesViewer:RegisterGuide(
  'Test',
  {},
  [[
step
accept A Threat Within##783
turnin Deputy Willem##823
]])
"#;
        let regions = parser.parse_document(source);
        assert_eq!(regions.len(), 1);
        assert!(regions[0].tree.is_some());
        let tree = regions[0].tree.as_ref().unwrap();
        assert!(!tree.root_node().has_error());
    }
}
