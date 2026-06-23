use std::error::Error;
use std::fs;
use std::path::Path;

use ironworks::{
    Ironworks,
    excel::{Excel, Field},
    file::exh::{ColumnDefinition, SheetKind},
    sestring::{Payload, SeString},
    sqpack::{Install, SqPack},
};

use crate::exd_schema::field_names;

mod ast;
mod binary;
mod contract;
mod language;
mod package;
mod shards;

use contract::{DEFAULT_SOURCE_SCOPES, DEFAULT_TALK_SHEET, DEFAULT_TALK_TEXT_COLUMNS, ScopeMode};
use language::BttLanguage;
use package::{
    bundle_diagnostic, read_game_version, require_game_install_root, write_root_diagnostics,
    write_source_bundle,
};
use shards::{DialogueRecordRef, SourceBundleBuilder, SourceRecordIdentityRef, StructureRecordRef};

#[derive(Debug)]
pub struct Options {
    pub game_path: String,
    pub output: String,
    pub languages: Option<Vec<String>>,
    pub sheets: Option<Vec<String>>,
}

impl Options {
    pub fn parse(args: &[String]) -> Result<Self, Box<dyn Error>> {
        if args.len() < 2 {
            return Err("BTT dialogue export requires a game path before --btt-dialogue.".into());
        }

        let mut output = String::from("output-btt-source");
        let mut languages = None;
        let mut sheets = None;
        let mut index = 2;

        while index < args.len() {
            match args[index].as_str() {
                "--btt-dialogue" => {
                    index += 1;
                }
                "--output" => {
                    output = required_value(args, index, "--output")?.to_string();
                    index += 2;
                }
                "--languages" => {
                    languages = Some(split_csv_arg(required_value(args, index, "--languages")?));
                    index += 2;
                }
                "--sheets" => {
                    sheets = Some(split_csv_arg(required_value(args, index, "--sheets")?));
                    index += 2;
                }
                other => return Err(format!("Unknown --btt-dialogue option: {other}").into()),
            }
        }

        Ok(Self {
            game_path: args[1].clone(),
            output,
            languages,
            sheets,
        })
    }
}

pub fn export(options: Options) -> Result<(), Box<dyn Error>> {
    let game_path = require_game_install_root(&options.game_path)?;
    let install = Install::at(&game_path);
    let ironworks = Ironworks::new().with_resource(SqPack::new(install));
    let languages = select_languages(&ironworks, options.languages.as_deref())?;
    let mut excel = Excel::new(ironworks);
    let game_version = read_game_version(&game_path).unwrap_or_else(|| "unknown".to_string());
    let scope_mode = if options.sheets.is_some() {
        ScopeMode::ExplicitSheets
    } else {
        ScopeMode::DefaultScopes
    };

    fs::create_dir_all(&options.output)?;
    let mut diagnostics = Vec::new();

    for language in languages {
        excel.set_default_language(language.ironworks());
        let language_code = language.code();
        let sheets = select_sheets(&excel, options.sheets.as_deref())?;
        let mut bundle = SourceBundleBuilder::new();

        for sheet_name in &sheets {
            export_sheet(&excel, sheet_name, &mut bundle).map_err(|error| {
                format!(
                    "Failed to export BTT dialogue sheet {sheet_name} for {language_code}: {error}"
                )
            })?;
        }

        let summary = write_source_bundle(
            Path::new(&options.output),
            language_code,
            &game_version,
            scope_mode,
            &sheets,
            bundle,
        )?;
        diagnostics.push(bundle_diagnostic(
            language_code,
            &game_version,
            scope_mode,
            sheets.len(),
            &summary,
        ));
    }

    write_root_diagnostics(Path::new(&options.output), diagnostics)?;
    Ok(())
}

fn select_languages(
    ironworks: &Ironworks,
    requested: Option<&[String]>,
) -> Result<Vec<BttLanguage>, Box<dyn Error>> {
    let available = language::available_languages(ironworks)?;
    if let Some(requested) = requested {
        let mut requested_codes = Vec::new();
        for code in requested {
            let code = code.as_str();
            if requested_codes.contains(&code) {
                return Err(format!("Duplicate language code: {code}").into());
            }

            let language = language::from_code(code)
                .ok_or_else(|| format!("Unsupported BTT dialogue language code: {code}"))?;
            if !available.contains(&language) {
                return Err(format!("Language is not available in this client: {code}").into());
            }

            requested_codes.push(code);
        }

        return Ok(available
            .into_iter()
            .filter(|language| requested_codes.iter().any(|code| *code == language.code()))
            .collect());
    }

    Ok(available)
}

fn select_sheets(
    excel: &Excel,
    requested: Option<&[String]>,
) -> Result<Vec<String>, Box<dyn Error>> {
    if let Some(requested) = requested {
        return Ok(requested.to_vec());
    }

    let mut sheets = excel
        .list()?
        .iter()
        .map(|sheet| sheet.to_string())
        .collect::<Vec<_>>();
    sheets.retain(|sheet| {
        DEFAULT_SOURCE_SCOPES
            .iter()
            .any(|scope| scope.matches(sheet))
    });
    sheets.sort();
    Ok(sheets)
}

fn export_sheet(
    excel: &Excel,
    sheet_name: &str,
    bundle: &mut SourceBundleBuilder,
) -> Result<(), Box<dyn Error>> {
    let sheet = excel.sheet(sheet_name)?;
    let has_subrows = sheet.kind()? == SheetKind::Subrows;
    let mut columns = sheet.columns()?;
    columns.sort_by_key(|column| column.offset);
    let names = field_names(sheet_name)?.unwrap_or_else(|| {
        let mut generated = vec!["#".to_string()];
        generated.extend((0..columns.len()).map(|index| index.to_string()));
        generated
    });

    for row in sheet.into_iter() {
        let row = row?;
        let row_id = match has_subrows {
            true => format!("{}.{}", row.row_id(), row.subrow_id()),
            false => row.row_id().to_string(),
        };

        let mut fields = Vec::new();
        for (index, column) in columns.iter().enumerate() {
            let specifier = ColumnDefinition {
                kind: column.kind,
                offset: column.offset,
            };
            let field = row.field(&specifier)?;
            let name = names
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| index.to_string());
            fields.push((name, field));
        }

        if sheet_name == DEFAULT_TALK_SHEET {
            export_default_talk_row(bundle, sheet_name, &row_id, &fields)?;
            continue;
        }

        if is_dialogue_event_sheet(sheet_name) {
            export_event_dialogue_row(bundle, sheet_name, &row_id, &fields)?;
        }
    }

    Ok(())
}

fn export_default_talk_row(
    bundle: &mut SourceBundleBuilder,
    sheet_name: &str,
    row_id: &str,
    fields: &[(String, Field)],
) -> Result<(), Box<dyn Error>> {
    for column in DEFAULT_TALK_TEXT_COLUMNS {
        let field = require_named_field(fields, column, sheet_name, row_id)?;
        let Field::String(value) = field else {
            return Err(format!(
                "Expected DefaultTalk field to be a string: {sheet_name}:{row_id}:{column}"
            )
            .into());
        };

        let key = format!("{DEFAULT_TALK_SHEET}_{row_id}_{column}");
        push_source_text(bundle, sheet_name, row_id, column, &key, value.as_ref())?;
    }

    Ok(())
}

fn export_event_dialogue_row(
    bundle: &mut SourceBundleBuilder,
    sheet_name: &str,
    row_id: &str,
    fields: &[(String, Field)],
) -> Result<(), Box<dyn Error>> {
    let (_, key_field) = require_field_at(fields, 0, "dialogue key", sheet_name, row_id)?;
    let Field::String(key_field) = key_field else {
        return Err(
            format!("Expected dialogue key field to be a string: {sheet_name}:{row_id}:0").into(),
        );
    };
    let (text_column, text_field) =
        require_field_at(fields, 1, "dialogue text", sheet_name, row_id)?;
    let Field::String(text_field) = text_field else {
        return Err(format!(
            "Expected dialogue text field to be a string: {sheet_name}:{row_id}:1"
        )
        .into());
    };

    // Event keys become package identities. Macro payloads would make that
    // identity data-dependent, so fail instead of exporting an unstable key.
    let Some(key) = literal_sestring(key_field.as_ref())? else {
        return Err(format!(
            "Dialogue key field contains non-literal payload: {sheet_name}:{row_id}"
        )
        .into());
    };
    if key.trim().is_empty() {
        bundle.count_skipped_empty_key();
        return Ok(());
    }
    if key != key.trim() {
        return Err(format!(
            "Dialogue key field has surrounding whitespace: {sheet_name}:{row_id}"
        )
        .into());
    }

    push_source_text(
        bundle,
        sheet_name,
        row_id,
        text_column,
        &key,
        text_field.as_ref(),
    )
}

fn push_source_text(
    bundle: &mut SourceBundleBuilder,
    sheet_name: &str,
    row_id: &str,
    column: &str,
    key: &str,
    value: SeString<'_>,
) -> Result<(), Box<dyn Error>> {
    let has_text = !is_empty_string(value.as_ref())?;
    let identity = SourceRecordIdentityRef {
        sheet: sheet_name,
        row: row_id,
        column,
        key,
    };
    bundle.push_structure(StructureRecordRef { identity, has_text })?;

    if !has_text {
        return Ok(());
    }

    bundle.push_dialogue(DialogueRecordRef { identity, value })
}

fn is_dialogue_event_sheet(sheet_name: &str) -> bool {
    DEFAULT_SOURCE_SCOPES
        .iter()
        .any(|scope| scope.matches(sheet_name))
}

fn require_named_field<'a>(
    fields: &'a [(String, Field)],
    name: &str,
    sheet_name: &str,
    row_id: &str,
) -> Result<&'a Field, Box<dyn Error>> {
    fields
        .iter()
        .find(|(field_name, _)| field_name == name)
        .map(|(_, field)| field)
        .ok_or_else(|| format!("Missing expected field: {sheet_name}:{row_id}:{name}").into())
}

fn require_field_at<'a>(
    fields: &'a [(String, Field)],
    index: usize,
    role: &str,
    sheet_name: &str,
    row_id: &str,
) -> Result<(&'a str, &'a Field), Box<dyn Error>> {
    fields
        .get(index)
        .map(|(name, field)| (name.as_str(), field))
        .ok_or_else(|| format!("Missing {role} field: {sheet_name}:{row_id}:{index}").into())
}

fn is_empty_string(value: SeString<'_>) -> Result<bool, Box<dyn Error>> {
    // Event dialogue sheets use literal 0 as an empty-text sentinel.
    Ok(
        matches!(literal_sestring(value)?, Some(text) if text.trim().is_empty() || text.trim() == "0"),
    )
}

fn literal_sestring(value: SeString<'_>) -> Result<Option<String>, Box<dyn Error>> {
    let mut output = String::new();
    for payload in value.payloads() {
        match payload? {
            Payload::Text(text) => output.push_str(text.as_utf8()?),
            Payload::Macro(_) => return Ok(None),
        }
    }

    Ok(Some(output))
}

fn required_value<'a>(
    args: &'a [String],
    option_index: usize,
    option: &str,
) -> Result<&'a str, Box<dyn Error>> {
    args.get(option_index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("{option} requires a value").into())
}

fn split_csv_arg(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
