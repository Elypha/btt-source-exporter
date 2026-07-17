use std::error::Error;

use ironworks::{
    excel::{Excel, Field},
    file::exh::{ColumnDefinition, SheetKind},
    sestring::{Payload, SeString},
};

use crate::exd_schema::field_names;

use super::shards::{
    DialogueRecordRef, SourceBundleBuilder, SourceRecordIdentityRef, StructureRecordRef,
};
use super::source_model::SourceModel;

// sheet selection
// --------------------------------
pub(super) fn select_sheets(
    excel: &Excel,
    source_model: &SourceModel,
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
    sheets.retain(|sheet| source_model.matches_default_scope(sheet));
    sheets.sort();
    Ok(sheets)
}

// row extraction
// --------------------------------
pub(super) fn export_sheet(
    excel: &Excel,
    sheet_name: &str,
    source_model: &SourceModel,
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

        if let Some(text_columns) = source_model.standalone_talk_text_columns(sheet_name) {
            export_standalone_talk_row(bundle, sheet_name, &row_id, &fields, text_columns)?;
            continue;
        }

        if source_model.is_event_dialogue_sheet(sheet_name) {
            export_event_dialogue_row(bundle, sheet_name, &row_id, &fields)?;
        }
    }

    Ok(())
}

fn export_standalone_talk_row(
    bundle: &mut SourceBundleBuilder,
    sheet_name: &str,
    row_id: &str,
    fields: &[(String, Field)],
    text_columns: &[String],
) -> Result<(), Box<dyn Error>> {
    for column in text_columns {
        let field = require_named_field(fields, column, sheet_name, row_id)?;
        let Field::String(value) = field else {
            return Err(format!(
                "Expected standalone talk field to be a string: {sheet_name}:{row_id}:{column}"
            )
            .into());
        };

        let key = format!("{sheet_name}_{row_id}_{column}");
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

// source identity helpers
// --------------------------------
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
