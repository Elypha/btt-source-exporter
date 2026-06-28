import fs from "node:fs";
import path from "node:path";

import {
  sourceLanguageDefinitions,
  templateExpansionDiagnosticLimit,
} from "./contract.mjs";
import { createRecordCollector } from "./record-collector.mjs";
import { applyReleasePolicy } from "./release-policy.mjs";
import { writeDiagnostics, writeReleaseArtifacts } from "./release-writer.mjs";
import { readSourceBundle } from "./source-bundles.mjs";
import { logPrefix } from "./utils.mjs";

export function buildDialogueRelease(options) {
  const sourceRoot = path.resolve(requiredPath(options.sourceRoot, "--source-root"));
  const outputDir = path.resolve(requiredPath(options.output, "--output"));
  const buildNumber = requirePositiveInteger(options.buildNumber, "--build-number");
  const limit = options.limit == null ? null : requireNonNegativeInteger(options.limit, "--limit");
  const diagnoseTemplateExpansion = Boolean(options.diagnoseTemplateExpansion);

  if (!fs.existsSync(sourceRoot)) {
    throw new Error(`Missing source bundle root: ${sourceRoot}`);
  }

  const builtAt = process.env.BTT_BUILT_AT ?? new Date().toISOString();
  const sourceGameVersions = new Set();
  const diagnostics = createDiagnostics({
    builtAt,
    buildNumber,
    gameVersion: options.gameVersion,
    diagnoseTemplateExpansion,
  });

  const collector = createRecordCollector(diagnostics);
  for (const definition of sourceLanguageDefinitions) {
    const sourceBundleName = `${definition.code}.bttsrc.tar.zst`;
    const sourceBundlePath = path.join(sourceRoot, sourceBundleName);
    diagnostics.languages[definition.code] = createLanguageDiagnostics(definition, sourceBundleName);

    if (!fs.existsSync(sourceBundlePath)) {
      diagnostics.errors.push(`Missing source bundle for ${definition.code}: ${sourceBundleName}`);
      continue;
    }

    console.error(`${logPrefix} reading ${definition.code} source bundle from ${sourceBundleName}`);
    diagnostics.parser.sourceBundlesRead++;
    const bundle = readSourceBundle(definition, sourceBundlePath, sourceBundleName);
    diagnostics.errors.push(...bundle.errors);
    if (!bundle.manifest) continue;

    sourceGameVersions.add(bundle.manifest.gameVersion);
    recordBundleDiagnostics(diagnostics, definition, bundle);
    collector.addBundle(definition, bundle);
    console.error(
      `${logPrefix} finished ${definition.code}: `
      + `${diagnostics.languages[definition.code].textEntries} text entries`);
  }

  const release = applyReleasePolicy({
    diagnostics,
    collector,
    gameVersion: options.gameVersion ?? null,
    limit,
    sourceGameVersions,
    diagnoseTemplateExpansion,
  });

  if (diagnostics.errors.length > 0) {
    writeDiagnostics(outputDir, diagnostics);
    console.error(JSON.stringify({
      errors: diagnostics.errors.slice(0, 20),
      diagnosticsPath: displayPath(path.join(outputDir, "build-diagnostics.json")),
    }, null, 2));
    throw new Error("Dialogue package build failed.");
  }

  return writeReleaseArtifacts({
    outputDir,
    records: release.selectedEntries,
    buildNumber,
    gameVersion: release.gameVersion,
    builtAt,
    diagnostics,
  });
}

// diagnostics shape
// --------------------------------
function createDiagnostics({ builtAt, buildNumber, gameVersion, diagnoseTemplateExpansion }) {
  return {
    builtAt,
    buildNumber,
    gameVersion: gameVersion ?? null,
    languages: {},
    parser: {
      sourceBundlesRead: 0,
      structureRowsRead: 0,
      dialogueRowsRead: 0,
    },
    structure: {
      intl: {},
      simplifiedChinese: {},
      sourceCoverage: {},
      duplicates: [],
    },
    templateExpansion: {
      enabled: diagnoseTemplateExpansion,
      limit: templateExpansionDiagnosticLimit,
      textsWithControlFlow: 0,
      maxObservedVariants: 0,
      overflowCount: 0,
      malformedCount: 0,
      samples: [],
    },
    coverage: {},
    duplicates: [],
    errors: [],
    warnings: [],
    output: {},
  };
}

function createLanguageDiagnostics(definition, sourceBundlePath) {
  return {
    sourceBundlePath,
    structureRecords: 0,
    expectedTextKeys: 0,
    dialogueRecords: 0,
    emptyTextRecords: 0,
    skippedEmptyKeys: 0,
    textEntries: 0,
    structuralKeys: 0,
  };
}

function recordBundleDiagnostics(diagnostics, definition, bundle) {
  const languageDiagnostics = diagnostics.languages[definition.code];
  languageDiagnostics.gameVersion = bundle.manifest.gameVersion;
  languageDiagnostics.scopeMode = bundle.manifest.scopeMode;
  languageDiagnostics.sheetCount = bundle.manifest.sheetCount;
  languageDiagnostics.structureRecords = bundle.manifest.structureRecords;
  languageDiagnostics.dialogueRecords = bundle.manifest.dialogueRecords;
  languageDiagnostics.emptyTextRecords = bundle.manifest.emptyTextRecords;
  languageDiagnostics.skippedEmptyKeys = bundle.manifest.skippedEmptyKeys;
  diagnostics.parser.structureRowsRead += bundle.structureRows.length;
  diagnostics.parser.dialogueRowsRead += bundle.dialogueRows.length;
}

function displayPath(file) {
  return path.relative(process.cwd(), file) || ".";
}

// CLI option normalisation
// --------------------------------
function requiredPath(value, option) {
  if (value == null || value === "") throw new Error(`Pass ${option} <path>.`);
  return value;
}

function requirePositiveInteger(value, option) {
  const number = Number(value);
  if (!Number.isInteger(number) || number <= 0) {
    throw new Error(`Pass a positive integer ${option} <n>.`);
  }

  return number;
}

function requireNonNegativeInteger(value, option) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < 0) {
    throw new Error(`Pass a non-negative integer ${option} <n>.`);
  }

  return number;
}
