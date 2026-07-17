// source bundle manifest
// --------------------------------
pub(super) const SOURCE_BUNDLE_FORMAT: &str = "btt-dialogue-source-bundle";
pub(super) const SOURCE_BUNDLE_FORMAT_VERSION: u32 = 1;
pub(super) const SOURCE_BUNDLE_KIND: &str = "dialogue-source";
pub(super) const STRUCTURE_SCHEMA_VERSION: u16 = 1;
pub(super) const DIALOGUE_SCHEMA_VERSION: u16 = 1;
pub(super) const DIALOGUE_IR_ENCODING_VERSION: u32 = 2;

pub(super) const MANIFEST_FILE: &str = "manifest.json";
pub(super) const STRUCTURE_FILE_PATH: &str = "structure.bttbin";
pub(super) const DIALOGUE_FILE_PATH: &str = "dialogue.bttbin";

// source shard magic
// --------------------------------
pub(super) const STRUCTURE_MAGIC: &[u8; 16] = b"BTT-SRC-STRUCT\0\0";
pub(super) const DIALOGUE_MAGIC: &[u8; 16] = b"BTT-SRC-DIALOGUE";

// dialogue IR node tags
// --------------------------------
pub(super) const IR_SEQUENCE: u8 = 1;
pub(super) const IR_TEXT: u8 = 2;
pub(super) const IR_PLACEHOLDER: u8 = 3;
pub(super) const IR_PARAMETER: u8 = 4;
pub(super) const IR_PLAYER_NAME: u8 = 5;
pub(super) const IR_IF: u8 = 6;
pub(super) const IR_SWITCH: u8 = 7;

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
}
