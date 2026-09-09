use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::config::{self, LspConfig};
use crate::db::index::Database;
use crate::guide::parser::{DocumentParser, GuideRegion};
use crate::guide::semantic;
use crate::guide::types::GuideModel;
use crate::providers::{
    code_actions, completion, definition, diagnostics, hover, position, references,
};

pub struct Backend {
    client: Client,
    db: tokio::sync::OnceCell<Arc<Database>>,
    documents: DashMap<Url, DocumentState>,
    parser: tokio::sync::Mutex<DocumentParser>,
    workspace_root: tokio::sync::OnceCell<std::path::PathBuf>,
}

struct DocumentState {
    source: String,
    guides: Vec<ParsedGuide>,
}

struct ParsedGuide {
    #[allow(dead_code)]
    region: GuideRegion,
    model: GuideModel,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            db: tokio::sync::OnceCell::new(),
            documents: DashMap::new(),
            parser: tokio::sync::Mutex::new(DocumentParser::new()),
            workspace_root: tokio::sync::OnceCell::new(),
        }
    }

    async fn load_database(&self, workspace_root: Option<&std::path::Path>) -> Option<Arc<Database>> {
        let config = LspConfig::default();
        let (pf_path, epoch_path) = config::resolve_database_paths(&config, workspace_root)?;

        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "loading database from {} and {}",
                    pf_path.display(),
                    epoch_path.display()
                ),
            )
            .await;

        match Database::load(&pf_path, &epoch_path) {
            Ok(db) => {
                self.client
                    .log_message(
                        MessageType::INFO,
                        format!(
                            "database loaded: {} quests, {} units, {} items, {} objects, {} zones",
                            db.quests.len(),
                            db.units.len(),
                            db.items.len(),
                            db.objects.len(),
                            db.zones.len()
                        ),
                    )
                    .await;
                Some(Arc::new(db))
            }
            Err(e) => {
                self.client
                    .log_message(MessageType::ERROR, format!("failed to load database: {}", e))
                    .await;
                None
            }
        }
    }

    async fn index_workspace(&self) {
        let Some(root) = self.workspace_root.get() else {
            return;
        };

        let guide_dirs: Vec<std::path::PathBuf> = ["Guides"]
            .iter()
            .map(|d| root.join(d))
            .filter(|p| p.exists())
            .collect();

        if guide_dirs.is_empty() {
            return;
        }

        let mut count = 0;
        for dir in &guide_dirs {
            let Ok(walker) = walkdir(dir) else {
                continue;
            };
            for path in walker {
                if !path.to_string_lossy().ends_with(".lua") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if !text.contains("RegisterGuide") {
                    continue;
                }
                let uri = match Url::from_file_path(&path) {
                    Ok(u) => u,
                    Err(_) => continue,
                };
                if self.documents.contains_key(&uri) {
                    continue;
                }
                self.parse_and_store_quiet(&uri, text).await;
                count += 1;
            }
        }

        self.client
            .log_message(
                MessageType::INFO,
                format!("indexed {} guide files from workspace", count),
            )
            .await;
    }

    async fn parse_and_store_quiet(&self, uri: &Url, text: String) {
        let mut parser = self.parser.lock().await;
        let regions = parser.parse_document(&text);
        drop(parser);

        let guides: Vec<ParsedGuide> = regions
            .into_iter()
            .map(|region| {
                let model = semantic::build_model(&region);
                ParsedGuide { region, model }
            })
            .collect();

        self.documents.insert(
            uri.clone(),
            DocumentState {
                source: text,
                guides,
            },
        );
    }

    async fn parse_and_store(&self, uri: &Url, text: String) {
        let mut parser = self.parser.lock().await;
        let regions = parser.parse_document(&text);
        drop(parser);

        let mut all_diags = Vec::new();

        let guides: Vec<ParsedGuide> = regions
            .into_iter()
            .map(|region| {
                let model = semantic::build_model(&region);

                if let Some(db) = self.db.get() {
                    let diags =
                        diagnostics::diagnose(&model, db, &text, region.content_offset);
                    all_diags.extend(diags);
                }

                ParsedGuide { region, model }
            })
            .collect();

        self.documents.insert(
            uri.clone(),
            DocumentState {
                source: text,
                guides,
            },
        );

        self.client
            .publish_diagnostics(uri.clone(), all_diags, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let workspace_root = params
            .root_uri
            .as_ref()
            .and_then(|u| u.to_file_path().ok());

        let root_ref = workspace_root.as_deref();

        if let Some(root) = workspace_root.as_ref() {
            let _ = self.workspace_root.set(root.clone());
        }

        if let Some(db) = self.load_database(root_ref).await {
            let _ = self.db.set(db);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        "|".to_string(),
                        "#".to_string(),
                        " ".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "zygorguide-lsp initialized")
            .await;
        self.index_workspace().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.parse_and_store(&uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.parse_and_store(&uri, change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        let db = match self.db.get() {
            Some(db) => db,
            None => return Ok(None),
        };

        let offset = position::position_to_byte_offset(&doc.source, pos);
        for guide in &doc.guides {
            let start = guide.region.content_offset;
            let end = start + guide.region.content.len();
            if offset >= start && offset < end {
                let content_offset = offset - start;
                let hit = position::find_ref_at_offset(&guide.model, content_offset);
                return Ok(hover::hover_info(&hit, db));
            }
        }
        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let offset = position::position_to_byte_offset(&doc.source, pos);
        for guide in &doc.guides {
            let start = guide.region.content_offset;
            let end = start + guide.region.content.len();
            if offset >= start && offset < end {
                let content_offset = offset - start;
                let hit = position::find_ref_at_offset(&guide.model, content_offset);
                return Ok(definition::goto_definition(
                    &hit,
                    &guide.model,
                    &doc.source,
                    guide.region.content_offset,
                    uri,
                ));
            }
        }
        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };

        let offset = position::position_to_byte_offset(&doc.source, pos);
        let models: Vec<_> = doc
            .guides
            .iter()
            .map(|g| (g.model.clone(), g.region.content_offset))
            .collect();

        for guide in &doc.guides {
            let start = guide.region.content_offset;
            let end = start + guide.region.content.len();
            if offset >= start && offset < end {
                let content_offset = offset - start;
                let hit = position::find_ref_at_offset(&guide.model, content_offset);
                return Ok(references::find_references(
                    &hit,
                    &models,
                    &doc.source,
                    uri,
                ));
            }
        }
        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        let db = match self.db.get() {
            Some(db) => db,
            None => return Ok(None),
        };

        let line_text = match position::get_line_up_to_cursor(&doc.source, pos) {
            Some(l) => l,
            None => return Ok(None),
        };

        let offset = position::position_to_byte_offset(&doc.source, pos);
        let current_model = doc.guides.iter().find(|g| {
            let start = g.region.content_offset;
            let end = start + g.region.content.len();
            offset >= start && offset < end
        });

        Ok(completion::complete(
            &line_text,
            db,
            current_model.map(|g| &g.model),
        ))
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        let request_range = params.range;

        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return Ok(None),
        };
        let db = match self.db.get() {
            Some(db) => db,
            None => return Ok(None),
        };

        let mut all_actions = Vec::new();
        for guide in &doc.guides {
            let actions = code_actions::code_actions(
                &guide.model,
                db,
                &doc.source,
                guide.region.content_offset,
                uri,
                request_range,
            );
            all_actions.extend(actions);
        }

        if all_actions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(all_actions))
        }
    }
}

fn walkdir(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk_recursive(dir, &mut files)?;
    Ok(files)
}

fn walk_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_recursive(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}
