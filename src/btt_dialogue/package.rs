use std::error::Error;
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tar::{Builder, Header};
use zstd::stream::Encoder;

use super::binary::checked_u64;
use super::contract::{
    DIALOGUE_FILE_PATH, DIALOGUE_IR_ENCODING_VERSION, DIALOGUE_SCHEMA_VERSION, MANIFEST_FILE,
    SOURCE_BUNDLE_FORMAT, SOURCE_BUNDLE_FORMAT_VERSION, SOURCE_BUNDLE_KIND, STRUCTURE_FILE_PATH,
    STRUCTURE_SCHEMA_VERSION, ScopeMode,
};
use super::shards::SourceBundleBuilder;

// source bundle archive
// --------------------------------
pub(super) struct BundleSummary {
    pub(super) dialogue_records: usize,
    pub(super) structure_records: usize,
    pub(super) empty_text_records: usize,
    pub(super) skipped_empty_keys: usize,
}

pub(super) fn write_source_bundle(
    output_dir: &Path,
    language: &str,
    game_version: &str,
    scope_mode: ScopeMode,
    source_scopes: &[String],
    sheets: &[String],
    bundle: SourceBundleBuilder,
) -> Result<BundleSummary, Box<dyn Error>> {
    let structure_records = bundle.structure_records();
    let dialogue_records = bundle.dialogue_records();
    let empty_text_records = bundle.empty_text_records()?;
    let skipped_empty_keys = bundle.skipped_empty_keys();

    let structure_bytes = bundle.structure_bytes()?;
    let dialogue_bytes = bundle.dialogue_bytes()?;

    let manifest = SourceBundleManifest {
        format: SOURCE_BUNDLE_FORMAT,
        format_version: SOURCE_BUNDLE_FORMAT_VERSION,
        kind: SOURCE_BUNDLE_KIND,
        language,
        game_version,
        structure_schema_version: u32::from(STRUCTURE_SCHEMA_VERSION),
        dialogue_schema_version: u32::from(DIALOGUE_SCHEMA_VERSION),
        dialogue_ir_encoding_version: DIALOGUE_IR_ENCODING_VERSION,
        scope_mode: scope_mode.as_str(),
        source_scopes: match scope_mode {
            ScopeMode::DefaultScopes => source_scopes.to_vec(),
            ScopeMode::ExplicitSheets => Vec::new(),
        },
        sheets: sheets.to_vec(),
        sheet_count: sheets.len(),
        structure_records,
        dialogue_records,
        empty_text_records,
        skipped_empty_keys,
    };
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    manifest_bytes.push(b'\n');

    let archive_path = output_dir.join(format!("{language}.bttsrc.tar.zst"));
    write_tar_zstd(
        &archive_path,
        &[
            (MANIFEST_FILE, manifest_bytes.as_slice()),
            (STRUCTURE_FILE_PATH, structure_bytes.as_slice()),
            (DIALOGUE_FILE_PATH, dialogue_bytes.as_slice()),
        ],
    )?;

    Ok(BundleSummary {
        dialogue_records,
        structure_records,
        empty_text_records,
        skipped_empty_keys,
    })
}

#[derive(Serialize)]
struct SourceBundleManifest<'a> {
    format: &'a str,
    #[serde(rename = "formatVersion")]
    format_version: u32,
    kind: &'a str,
    language: &'a str,
    #[serde(rename = "gameVersion")]
    game_version: &'a str,
    #[serde(rename = "structureSchemaVersion")]
    structure_schema_version: u32,
    #[serde(rename = "dialogueSchemaVersion")]
    dialogue_schema_version: u32,
    #[serde(rename = "dialogueIrEncodingVersion")]
    dialogue_ir_encoding_version: u32,
    #[serde(rename = "scopeMode")]
    scope_mode: &'a str,
    #[serde(rename = "sourceScopes")]
    source_scopes: Vec<String>,
    sheets: Vec<String>,
    #[serde(rename = "sheetCount")]
    sheet_count: usize,
    #[serde(rename = "structureRecords")]
    structure_records: usize,
    #[serde(rename = "dialogueRecords")]
    dialogue_records: usize,
    #[serde(rename = "emptyTextRecords")]
    empty_text_records: usize,
    #[serde(rename = "skippedEmptyKeys")]
    skipped_empty_keys: usize,
}

// export-run diagnostics
// --------------------------------
#[derive(Deserialize, Serialize)]
pub(super) struct SourceBundleDiagnostic {
    language: String,
    format: String,
    #[serde(rename = "formatVersion")]
    format_version: u32,
    #[serde(rename = "gameVersion")]
    game_version: String,
    #[serde(rename = "scopeMode")]
    scope_mode: String,
    #[serde(rename = "sheetCount")]
    sheet_count: usize,
    records: usize,
    #[serde(rename = "structureRecords")]
    structure_records: usize,
    #[serde(rename = "emptyTextRecords")]
    empty_text_records: usize,
    #[serde(rename = "skippedEmptyKeys")]
    skipped_empty_keys: usize,
    errors: Vec<String>,
}

pub(super) fn write_run_diagnostics(
    output_dir: &Path,
    diagnostics: Vec<SourceBundleDiagnostic>,
) -> Result<(), Box<dyn Error>> {
    let diagnostics_path = output_dir.join(diagnostics_file_name(&diagnostics)?);
    fs::write(
        diagnostics_path,
        format!("{}\n", serde_json::to_string_pretty(&diagnostics)?),
    )?;

    Ok(())
}

fn diagnostics_file_name(diagnostics: &[SourceBundleDiagnostic]) -> Result<String, Box<dyn Error>> {
    let languages = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.language.as_str())
        .collect::<Vec<_>>();
    if languages.is_empty() {
        return Err("No languages were exported; diagnostics file name would be empty.".into());
    }

    Ok(format!("{}.diagnostics.json", languages.join(",")))
}

pub(super) fn bundle_diagnostic(
    language: &str,
    game_version: &str,
    scope_mode: ScopeMode,
    sheet_count: usize,
    summary: &BundleSummary,
) -> SourceBundleDiagnostic {
    SourceBundleDiagnostic {
        language: language.to_string(),
        format: SOURCE_BUNDLE_FORMAT.to_string(),
        format_version: SOURCE_BUNDLE_FORMAT_VERSION,
        game_version: game_version.to_string(),
        scope_mode: scope_mode.as_str().to_string(),
        sheet_count,
        records: summary.dialogue_records,
        structure_records: summary.structure_records,
        empty_text_records: summary.empty_text_records,
        skipped_empty_keys: summary.skipped_empty_keys,
        errors: Vec::new(),
    }
}

// client install metadata
// --------------------------------
pub(super) fn read_game_version(game_path: &Path) -> Option<String> {
    fs::read_to_string(game_path.join("game/ffxivgame.ver"))
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

pub(super) fn require_game_install_root(game_path: &str) -> Result<PathBuf, Box<dyn Error>> {
    let root = Path::new(game_path);
    if root.join("game/sqpack").exists() {
        return Ok(root.to_path_buf());
    }

    Err(format!(
        "BTT source export requires a client install root that contains the game folder: {}",
        root.display()
    )
    .into())
}

// deterministic tar.zst writer
// --------------------------------
fn write_tar_zstd(path: &Path, files: &[(&str, &[u8])]) -> Result<(), Box<dyn Error>> {
    let file = File::create(path)?;
    let encoder = Encoder::new(file, 10)?;
    let mut tar = Builder::new(encoder);
    for (name, data) in files {
        append_tar_file(&mut tar, name, data)?;
    }

    let encoder = tar.into_inner()?;
    encoder.finish()?;
    Ok(())
}

fn append_tar_file<W: Write>(
    tar: &mut Builder<W>,
    name: &str,
    data: &[u8],
) -> Result<(), Box<dyn Error>> {
    let mut header = Header::new_gnu();
    header.set_size(checked_u64(data.len(), "tar entry byte length")?);
    // Fixed metadata keeps source bundle hashes reproducible.
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    tar.append_data(&mut header, name, Cursor::new(data))?;
    Ok(())
}
