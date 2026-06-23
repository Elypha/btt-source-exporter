use std::error::Error;
use std::fs::{self, File};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::{Builder, Header};
use zstd::stream::Encoder;

use super::binary::checked_u64;
use super::contract::{
    AST_ENCODING_VERSION, DIAGNOSTICS_FILE, DIALOGUE_FILE_PATH, DIALOGUE_FILE_ROLE,
    DIALOGUE_SCHEMA_VERSION, MANIFEST_FILE, SOURCE_BUNDLE_FORMAT, SOURCE_BUNDLE_FORMAT_VERSION,
    SOURCE_BUNDLE_KIND, STRUCTURE_FILE_PATH, STRUCTURE_FILE_ROLE, STRUCTURE_SCHEMA_VERSION,
    ScopeMode,
};
use super::language;
use super::shards::SourceBundleBuilder;

pub(super) struct BundleSummary {
    pub(super) dialogue_records: usize,
    pub(super) structure_records: usize,
    pub(super) empty_text_records: usize,
    pub(super) skipped_empty_keys: usize,
    pub(super) bundle_path: String,
    pub(super) bundle_bytes: u64,
    pub(super) bundle_sha256: String,
}

pub(super) fn write_source_bundle(
    output_dir: &Path,
    language: &str,
    game_version: &str,
    scope_mode: ScopeMode,
    sheets: &[String],
    bundle: SourceBundleBuilder,
) -> Result<BundleSummary, Box<dyn Error>> {
    let structure_records = bundle.structure_records();
    let dialogue_records = bundle.dialogue_records();
    let empty_text_records = bundle.empty_text_records()?;
    let skipped_empty_keys = bundle.skipped_empty_keys();

    let structure_bytes = bundle.structure_bytes()?;
    let dialogue_bytes = bundle.dialogue_bytes()?;
    let structure_hash = sha256_bytes(&structure_bytes);
    let dialogue_hash = sha256_bytes(&dialogue_bytes);

    let files = vec![
        SourceBundleFile {
            role: STRUCTURE_FILE_ROLE,
            path: STRUCTURE_FILE_PATH,
            bytes: checked_u64(structure_bytes.len(), "structure shard byte length")?,
            sha256: structure_hash,
            record_count: structure_records,
        },
        SourceBundleFile {
            role: DIALOGUE_FILE_ROLE,
            path: DIALOGUE_FILE_PATH,
            bytes: checked_u64(dialogue_bytes.len(), "dialogue shard byte length")?,
            sha256: dialogue_hash,
            record_count: dialogue_records,
        },
    ];

    let manifest = SourceBundleManifest {
        format: SOURCE_BUNDLE_FORMAT,
        format_version: SOURCE_BUNDLE_FORMAT_VERSION,
        kind: SOURCE_BUNDLE_KIND,
        language,
        game_version,
        structure_schema_version: u32::from(STRUCTURE_SCHEMA_VERSION),
        dialogue_schema_version: u32::from(DIALOGUE_SCHEMA_VERSION),
        ast_encoding_version: AST_ENCODING_VERSION,
        scope_mode: scope_mode.as_str(),
        source_scopes: scope_mode.source_scope_names(),
        sheets: sheets.to_vec(),
        sheet_count: sheets.len(),
        structure_records,
        dialogue_records,
        empty_text_records,
        skipped_empty_keys,
        files,
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

    let bundle_bytes = fs::metadata(&archive_path)?.len();
    let bundle_sha256 = sha256_file(&archive_path)?;

    Ok(BundleSummary {
        dialogue_records,
        structure_records,
        empty_text_records,
        skipped_empty_keys,
        bundle_path: archive_path.to_string_lossy().replace('\\', "/"),
        bundle_bytes,
        bundle_sha256,
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
    #[serde(rename = "astEncodingVersion")]
    ast_encoding_version: u32,
    #[serde(rename = "scopeMode")]
    scope_mode: &'a str,
    #[serde(rename = "sourceScopes")]
    source_scopes: Vec<&'a str>,
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
    files: Vec<SourceBundleFile>,
}

#[derive(Serialize)]
struct SourceBundleFile {
    role: &'static str,
    path: &'static str,
    bytes: u64,
    sha256: String,
    #[serde(rename = "recordCount")]
    record_count: usize,
}

pub(super) fn write_root_diagnostics(
    output_dir: &Path,
    diagnostics: Vec<SourceBundleDiagnostic>,
) -> Result<(), Box<dyn Error>> {
    let diagnostics_path = output_dir.join(DIAGNOSTICS_FILE);
    // Different regional clients can be exported into the same source root, so
    // update only the languages produced by this run.
    let mut merged_diagnostics = if diagnostics_path.exists() {
        let existing = fs::read_to_string(&diagnostics_path)?;
        serde_json::from_str::<Vec<SourceBundleDiagnostic>>(&existing)?
    } else {
        Vec::new()
    };

    for diagnostic in diagnostics {
        let language = diagnostic.language.clone();
        merged_diagnostics.retain(|entry| entry.language != language);
        merged_diagnostics.push(diagnostic);
    }

    merged_diagnostics.sort_by(|left, right| {
        language::canonical_index(&left.language).cmp(&language::canonical_index(&right.language))
    });
    fs::write(
        diagnostics_path,
        format!("{}\n", serde_json::to_string_pretty(&merged_diagnostics)?),
    )?;

    Ok(())
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
        bundle: summary.bundle_path.clone(),
        bundle_bytes: summary.bundle_bytes,
        bundle_sha256: summary.bundle_sha256.clone(),
        errors: Vec::new(),
    }
}

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
    bundle: String,
    #[serde(rename = "bundleBytes")]
    bundle_bytes: u64,
    #[serde(rename = "bundleSha256")]
    bundle_sha256: String,
    errors: Vec<String>,
}

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

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    Ok(sha256_bytes(&bytes))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
