pub(super) const DEFAULT_TALK_SHEET: &str = "DefaultTalk";
pub(super) const DEFAULT_TALK_TEXT_COLUMNS: [&str; 3] = ["Text[0]", "Text[1]", "Text[2]"];

pub(super) const DEFAULT_SOURCE_SCOPES: [SourceScope; 4] = [
    SourceScope::Sheet(DEFAULT_TALK_SHEET),
    SourceScope::Folder("custom"),
    SourceScope::Folder("quest"),
    SourceScope::Folder("cut_scene"),
];

pub(super) const SOURCE_BUNDLE_FORMAT: &str = "btt-dialogue-source-bundle";
pub(super) const SOURCE_BUNDLE_FORMAT_VERSION: u32 = 1;
pub(super) const SOURCE_BUNDLE_KIND: &str = "dialogue-source";
pub(super) const STRUCTURE_SCHEMA_VERSION: u16 = 1;
pub(super) const DIALOGUE_SCHEMA_VERSION: u16 = 1;
pub(super) const AST_ENCODING_VERSION: u32 = 1;

pub(super) const MANIFEST_FILE: &str = "manifest.json";
pub(super) const DIAGNOSTICS_FILE: &str = "diagnostics.json";
pub(super) const STRUCTURE_FILE_ROLE: &str = "structure";
pub(super) const STRUCTURE_FILE_PATH: &str = "structure.bttbin";
pub(super) const DIALOGUE_FILE_ROLE: &str = "dialogue";
pub(super) const DIALOGUE_FILE_PATH: &str = "dialogue.bttbin";

pub(super) const STRUCTURE_MAGIC: &[u8; 16] = b"BTT-SRC-STRUCT\0\0";
pub(super) const DIALOGUE_MAGIC: &[u8; 16] = b"BTT-SRC-DIALOGUE";

pub(super) const AST_SESTRING: u8 = 1;
pub(super) const AST_TEXT: u8 = 2;
pub(super) const AST_MACRO: u8 = 3;
pub(super) const AST_U32: u8 = 4;
pub(super) const AST_TIME_PART: u8 = 5;
pub(super) const AST_STACK_COLOR: u8 = 6;
pub(super) const AST_UNARY: u8 = 7;
pub(super) const AST_BINARY: u8 = 8;
pub(super) const AST_UNKNOWN_EXPRESSION: u8 = 9;
pub(super) const AST_UNHANDLED_EXPRESSION: u8 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceScope {
    Sheet(&'static str),
    Folder(&'static str),
}

impl SourceScope {
    pub(super) fn manifest_name(self) -> &'static str {
        match self {
            Self::Sheet(name) | Self::Folder(name) => name,
        }
    }

    pub(super) fn matches(self, sheet_name: &str) -> bool {
        match self {
            Self::Sheet(name) => sheet_name == name,
            Self::Folder(name) => sheet_name
                .strip_prefix(name)
                .is_some_and(|suffix| suffix.starts_with('/')),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScopeMode {
    DefaultScopes,
    ExplicitSheets,
}

impl ScopeMode {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::DefaultScopes => "default-scopes",
            Self::ExplicitSheets => "explicit-sheets",
        }
    }

    pub(super) fn source_scope_names(self) -> Vec<&'static str> {
        match self {
            Self::DefaultScopes => DEFAULT_SOURCE_SCOPES
                .iter()
                .map(|scope| scope.manifest_name())
                .collect(),
            Self::ExplicitSheets => Vec::new(),
        }
    }
}
