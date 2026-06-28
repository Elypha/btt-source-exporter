import {
  intlStructuralLanguages,
  languageProperty,
  packageLanguageDefinitions,
  releaseKeyLanguages,
  sourceLanguageDefinitions,
} from "./contract.mjs";
import { recordTemplateExpansionDiagnostics } from "./dialogue-ir-compiler.mjs";
import { stringIsEmpty } from "./utils.mjs";

export function applyReleasePolicy({ diagnostics, collector, gameVersion, limit, sourceGameVersions, diagnoseTemplateExpansion }) {
  const resolvedGameVersion = resolveGameVersion(diagnostics, gameVersion, sourceGameVersions);
  diagnostics.gameVersion = resolvedGameVersion ?? "unknown";

  verifySourceCoverage(diagnostics, collector);
  verifyIntlStructure(diagnostics, collector);
  diagnoseRegionalStructure(diagnostics, collector, "zh-Hans", "simplifiedChinese", true);

  const selectedEntries = selectReleaseEntries(collector.entries, collector.structuralKeys, limit);
  recordCoverage(diagnostics, selectedEntries);
  if (diagnoseTemplateExpansion) recordTemplateExpansionDiagnostics(selectedEntries, diagnostics);
  recordReleaseFailures(diagnostics);

  return { gameVersion: resolvedGameVersion, selectedEntries };
}

// release identity
// --------------------------------
function resolveGameVersion(diagnostics, gameVersion, sourceGameVersions) {
  const sourceGameVersionValues = [...sourceGameVersions]
    .filter((version) => !stringIsEmpty(version))
    .sort((left, right) => left.localeCompare(right, "en"));

  if (sourceGameVersionValues.length > 1) {
    diagnostics.warnings.push(`Source bundle gameVersion values differ: ${sourceGameVersionValues.join(", ")}.`);
  }

  if (gameVersion != null) return gameVersion;
  if (sourceGameVersionValues.length === 1) return sourceGameVersionValues[0];
  if (sourceGameVersionValues.length === 0) {
    diagnostics.errors.push("No source bundle gameVersion values found; pass --game-version explicitly.");
  } else {
    diagnostics.errors.push("Source bundle gameVersion values differ; pass --game-version explicitly.");
  }

  return null;
}

// source structure gates
// --------------------------------
function verifySourceCoverage(diagnostics, collector) {
  for (const definition of sourceLanguageDefinitions) {
    const expected = collector.expectedTextKeys.get(definition.code) ?? new Set();
    const actual = collector.emittedTextKeys.get(definition.code) ?? new Set();
    const diff = compareSets(expected, actual);
    diagnostics.structure.sourceCoverage[definition.code] = diff;
    if (diff.missingCount > 0 || diff.extraCount > 0) {
      diagnostics.errors.push(`Source coverage mismatch for ${definition.code}: ${diff.missingCount} missing, ${diff.extraCount} extra.`);
    }
  }
}

function verifyIntlStructure(diagnostics, collector) {
  const reference = collector.structuralKeys.get("ja") ?? new Set();
  for (const language of intlStructuralLanguages) {
    const current = collector.structuralKeys.get(language) ?? new Set();
    const diff = compareSets(reference, current);
    diagnostics.structure.intl[language] = diff;
    if (diff.missingCount > 0 || diff.extraCount > 0) {
      diagnostics.errors.push(`International structural mismatch for ${language}: ${diff.missingCount} missing, ${diff.extraCount} extra.`);
    }
  }
}

function diagnoseRegionalStructure(diagnostics, collector, language, name, failOnMissingBase) {
  const reference = collector.structuralKeys.get("ja") ?? new Set();
  const current = collector.structuralKeys.get(language);
  if (!current) {
    diagnostics.structure[name] = { missingSource: true };
    return;
  }

  const diff = compareSets(reference, current);
  diagnostics.structure[name] = diff;
  if (failOnMissingBase && diff.missingCount > 0) {
    diagnostics.errors.push(`${language} is missing ${diff.missingCount} base structural keys.`);
  }
}

// entry selection and summaries
// --------------------------------
function selectReleaseEntries(entries, structuralKeys, limit) {
  const baseKeySet = new Set([
    ...releaseKeyLanguages.flatMap((language) => [...(structuralKeys.get(language) ?? [])]),
  ]);

  const sortedEntries = [...entries.values()]
    .filter((entry) => releaseKeyLanguages.some((language) => Boolean(entry[languageProperty(language)])))
    .filter((entry) => baseKeySet.has(entry.key))
    .map((entry) => ({ entry, keyBytes: Buffer.from(entry.key, "utf8") }))
    .sort((left, right) => Buffer.compare(left.keyBytes, right.keyBytes))
    .map((item) => item.entry);

  return limit == null ? sortedEntries : sortedEntries.slice(0, limit);
}

function recordCoverage(diagnostics, records) {
  for (const definition of packageLanguageDefinitions) {
    let present = 0;
    for (const entry of records) {
      if (entry[definition.property]) present++;
    }

    diagnostics.coverage[definition.code] = {
      present,
      missing: records.length - present,
      total: records.length,
    };
  }
}

// release failure policy
// --------------------------------
function recordReleaseFailures(diagnostics) {
  if (diagnostics.templateExpansion.overflowCount > 0 || diagnostics.templateExpansion.malformedCount > 0) {
    diagnostics.warnings.push(
      `Template control-flow diagnostics found ${diagnostics.templateExpansion.overflowCount} over-limit texts `
      + `and ${diagnostics.templateExpansion.malformedCount} malformed texts. These are diagnostic-only until `
      + "the matcher has an expression-aware template engine.");
  }

  if (diagnostics.duplicates.length > 0) {
    diagnostics.errors.push(`${diagnostics.duplicates.length} duplicate key/language conflicts found.`);
  }

  if (diagnostics.structure.duplicates.length > 0) {
    diagnostics.errors.push(`${diagnostics.structure.duplicates.length} duplicate structure keys found.`);
  }
}

function compareSets(reference, current) {
  const missing = [];
  const extra = [];
  let missingCount = 0;
  let extraCount = 0;

  for (const value of reference) {
    if (!current.has(value)) {
      missingCount++;
      if (missing.length < 20) missing.push(value);
    }
  }

  for (const value of current) {
    if (!reference.has(value)) {
      extraCount++;
      if (extra.length < 20) extra.push(value);
    }
  }

  return { referenceCount: reference.size, currentCount: current.size, missingCount, extraCount, missing, extra };
}
