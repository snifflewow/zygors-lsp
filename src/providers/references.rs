use tower_lsp::lsp_types::*;

use super::position::{byte_range_to_lsp, HitResult};
use crate::guide::types::GuideModel;

pub fn find_references(
    hit: &HitResult,
    models: &[(GuideModel, usize)],
    source: &str,
    uri: &Url,
) -> Option<Vec<Location>> {
    match hit {
        HitResult::Entity(eref) => {
            let id = eref.id;
            let mut locations = Vec::new();
            for (model, content_offset) in models {
                for r in &model.entity_refs {
                    if r.id == id {
                        let range = byte_range_to_lsp(source, *content_offset, &r.byte_range);
                        locations.push(Location {
                            uri: uri.clone(),
                            range,
                        });
                    }
                }
            }
            if locations.is_empty() {
                None
            } else {
                Some(locations)
            }
        }
        HitResult::Quest(qref) => {
            let id = qref.quest_id;
            let mut locations = Vec::new();
            for (model, content_offset) in models {
                for r in &model.quest_refs {
                    if r.quest_id == id {
                        let range = byte_range_to_lsp(source, *content_offset, &r.byte_range);
                        locations.push(Location {
                            uri: uri.clone(),
                            range,
                        });
                    }
                }
                for r in &model.entity_refs {
                    if r.id == id {
                        let range = byte_range_to_lsp(source, *content_offset, &r.byte_range);
                        locations.push(Location {
                            uri: uri.clone(),
                            range,
                        });
                    }
                }
            }
            if locations.is_empty() {
                None
            } else {
                Some(locations)
            }
        }
        HitResult::Label(lref) => {
            let name = &lref.name;
            let mut locations = Vec::new();
            for (model, content_offset) in models {
                for r in &model.label_refs {
                    if r.name == *name {
                        let range = byte_range_to_lsp(source, *content_offset, &r.byte_range);
                        locations.push(Location {
                            uri: uri.clone(),
                            range,
                        });
                    }
                }
            }
            if locations.is_empty() {
                None
            } else {
                Some(locations)
            }
        }
        _ => None,
    }
}
