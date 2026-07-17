use std::{collections::HashSet, error::Error};

use serde::Deserialize;

const SOURCE_MODEL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/dialogue-sources.json"
));

#[derive(Debug)]
pub(super) struct SourceModel {
    sources: Vec<SourceDefinition>,
}

impl SourceModel {
    pub(super) fn load_default() -> Result<Self, Box<dyn Error>> {
        let model: SourceModelFile = serde_json::from_str(SOURCE_MODEL)
            .map_err(|error| format!("Failed to parse embedded dialogue source model: {error}"))?;

        if model.version != 1 {
            return Err(format!(
                "Unsupported dialogue source model version: {}",
                model.version
            )
            .into());
        }

        let source_model = Self {
            sources: model.sources,
        };
        source_model.validate()?;
        Ok(source_model)
    }

    pub(super) fn source_scope_names(&self) -> Vec<String> {
        self.sources
            .iter()
            .map(|source| source.scope_name().to_string())
            .collect()
    }

    pub(super) fn matches_default_scope(&self, sheet_name: &str) -> bool {
        self.sources.iter().any(|source| source.matches(sheet_name))
    }

    pub(super) fn standalone_talk_text_columns(&self, sheet_name: &str) -> Option<&[String]> {
        self.sources.iter().find_map(|source| match source {
            SourceDefinition::StandaloneTalk {
                sheet,
                text_columns,
            } if sheet == sheet_name => Some(text_columns.as_slice()),
            _ => None,
        })
    }

    pub(super) fn is_event_dialogue_sheet(&self, sheet_name: &str) -> bool {
        self.sources.iter().any(|source| {
            matches!(source, SourceDefinition::EventFolder { .. }) && source.matches(sheet_name)
        })
    }

    fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.sources.is_empty() {
            return Err("Dialogue source model must contain sources.".into());
        }

        let mut scope_names = HashSet::new();
        for source in &self.sources {
            let scope_name = source.scope_name();
            if scope_name.trim().is_empty() {
                return Err("Dialogue source model contains an empty source scope.".into());
            }
            if !scope_names.insert(scope_name) {
                return Err(format!("Duplicate dialogue source scope: {scope_name}").into());
            }

            if let SourceDefinition::StandaloneTalk { text_columns, .. } = source {
                if text_columns.is_empty() {
                    return Err(format!(
                        "Standalone talk source {scope_name} must define textColumns."
                    )
                    .into());
                }
                let mut column_names = HashSet::new();
                for column in text_columns {
                    if !column_names.insert(column) {
                        return Err(format!(
                            "Standalone talk source {scope_name} contains duplicate text column: {column}"
                        )
                        .into());
                    }
                }
                if text_columns.iter().any(|column| column.trim().is_empty()) {
                    return Err(format!(
                        "Standalone talk source {scope_name} contains an empty text column."
                    )
                    .into());
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceModelFile {
    version: u32,
    sources: Vec<SourceDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum SourceDefinition {
    StandaloneTalk {
        sheet: String,
        #[serde(rename = "textColumns")]
        text_columns: Vec<String>,
    },
    EventFolder {
        folder: String,
    },
}

impl SourceDefinition {
    fn scope_name(&self) -> &str {
        match self {
            Self::StandaloneTalk { sheet, .. } => sheet,
            Self::EventFolder { folder } => folder,
        }
    }

    fn matches(&self, sheet_name: &str) -> bool {
        match self {
            Self::StandaloneTalk { sheet, .. } => sheet == sheet_name,
            Self::EventFolder { folder } => sheet_name
                .strip_prefix(folder)
                .is_some_and(|suffix| suffix.starts_with('/')),
        }
    }
}
