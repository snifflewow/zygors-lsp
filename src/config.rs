use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LspConfig {
    #[serde(default)]
    pub database_paths: Option<DatabasePaths>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DatabasePaths {
    pub pf_quest: Option<String>,
    pub pf_quest_epoch: Option<String>,
}

pub fn resolve_database_paths(
    config: &LspConfig,
    workspace_root: Option<&Path>,
) -> Option<(PathBuf, PathBuf)> {
    if let Some(db_paths) = &config.database_paths {
        let base = workspace_root.unwrap_or_else(|| Path::new("."));
        let pf = db_paths
            .pf_quest
            .as_ref()
            .map(|p| base.join(p))
            .unwrap_or_else(|| base.join("pfQuest"));
        let epoch = db_paths
            .pf_quest_epoch
            .as_ref()
            .map(|p| base.join(p))
            .unwrap_or_else(|| base.join("pfQuest-epoch"));
        if pf.exists() && epoch.exists() {
            return Some((pf, epoch));
        }
    }

    if let Some(root) = workspace_root {
        for dir in [root, &root.join(".."), &root.join("../..")]  {
            let pf = dir.join("pfQuest");
            let epoch = dir.join("pfQuest-epoch");
            if pf.exists() && epoch.exists() {
                return Some((pf, epoch));
            }
        }
    }

    None
}
