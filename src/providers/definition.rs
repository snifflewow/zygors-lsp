use tower_lsp::lsp_types::*;

use super::position::{byte_range_to_lsp, HitResult};
use crate::guide::types::GuideModel;

pub fn goto_definition(
    hit: &HitResult,
    model: &GuideModel,
    source: &str,
    content_offset: usize,
    uri: &Url,
) -> Option<GotoDefinitionResponse> {
    match hit {
        HitResult::Label(lref) => {
            if model.sticky_starts.contains_key(&lref.name) {
                if let Some(target_ref) = find_label_target(model, &lref.name) {
                    let range = byte_range_to_lsp(source, content_offset, &target_ref);
                    return Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range,
                    }));
                }
            }

            if model.labels.contains_key(&lref.name) {
                if let Some(target_ref) = find_stickystart_target(model, &lref.name) {
                    let range = byte_range_to_lsp(source, content_offset, &target_ref);
                    return Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range,
                    }));
                }
            }
            None
        }
        _ => None,
    }
}

fn find_label_target(
    model: &GuideModel,
    name: &str,
) -> Option<std::ops::Range<usize>> {
    model
        .label_refs
        .iter()
        .find(|r| {
            r.name == name && model.labels.contains_key(&r.name)
        })
        .map(|r| r.byte_range.clone())
}

fn find_stickystart_target(
    model: &GuideModel,
    name: &str,
) -> Option<std::ops::Range<usize>> {
    model
        .label_refs
        .iter()
        .find(|r| {
            r.name == name && model.sticky_starts.contains_key(&r.name)
        })
        .map(|r| r.byte_range.clone())
}
