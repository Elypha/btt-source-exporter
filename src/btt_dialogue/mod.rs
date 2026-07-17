use std::error::Error;
use std::fs;
use std::path::Path;

use ironworks::{
    Ironworks,
    excel::Excel,
    sqpack::{Install, SqPack},
};

mod binary;
mod contract;
mod dialogue_ir;
mod language;
mod package;
mod shards;
mod source_extraction;
mod source_model;

use contract::ScopeMode;
use package::{
    bundle_diagnostic, read_game_version, require_game_install_root, write_run_diagnostics,
    write_source_bundle,
};
use shards::SourceBundleBuilder;
use source_extraction::{export_sheet, select_sheets};
use source_model::SourceModel;

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
            return Err("BTT dialogue export requires a game path.".into());
        }

        let mut output = String::from("output");
        let mut languages = None;
        let mut sheets = None;
        let mut index = 2;

        while index < args.len() {
            match args[index].as_str() {
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
                other => return Err(format!("Unknown option: {other}").into()),
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

// export orchestration
// --------------------------------
pub fn export(options: Options) -> Result<(), Box<dyn Error>> {
    let game_path = require_game_install_root(&options.game_path)?;
    let install = Install::at(&game_path);
    let ironworks = Ironworks::new().with_resource(SqPack::new(install));
    let languages = language::select_export_languages(&ironworks, options.languages.as_deref())?;
    if languages.is_empty() {
        return Err("No BTT dialogue languages are available in this client.".into());
    }

    let mut excel = Excel::new(ironworks);
    let source_model = SourceModel::load_default()?;
    let source_scopes = source_model.source_scope_names();
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
        let sheets = select_sheets(&excel, &source_model, options.sheets.as_deref())?;
        let mut bundle = SourceBundleBuilder::new();

        for sheet_name in &sheets {
            export_sheet(&excel, sheet_name, &source_model, &mut bundle).map_err(|error| {
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
            &source_scopes,
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

    write_run_diagnostics(Path::new(&options.output), diagnostics)?;
    Ok(())
}

// CLI option parsing helpers
// --------------------------------
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
