import { renderAuditText, renderCacheKey } from "./dialogue-ir-compiler.mjs";
import { cleanText } from "./utils.mjs";

// source bundle records to release entries
// --------------------------------
export function createRecordCollector(diagnostics) {
  const entries = new Map();
  const structuralKeys = new Map();
  const expectedTextKeys = new Map();
  const emittedTextKeys = new Map();
  const dialogueIds = new Map();
  let nextDialogueId = 1;

  return {
    entries,
    structuralKeys,
    expectedTextKeys,
    emittedTextKeys,
    addBundle(definition, bundle) {
      recordStructure(definition, bundle.structureRows);
      recordDialogue(definition, bundle.dialogueRows);
      diagnostics.languages[definition.code].structuralKeys = structuralKeys.get(definition.code)?.size ?? 0;
      diagnostics.languages[definition.code].expectedTextKeys = expectedTextKeys.get(definition.code)?.size ?? 0;
    },
  };

  // structure establishes the language-specific source key universe.
  function recordStructure(definition, rows) {
    const structure = getKeySet(structuralKeys, definition.code);
    const expected = getKeySet(expectedTextKeys, definition.code);
    for (const row of rows) {
      if (structure.has(row.key)) {
        if (diagnostics.structure.duplicates.length < 50) {
          diagnostics.structure.duplicates.push({ language: definition.code, key: row.key, source: row.source });
        }
      } else {
        structure.add(row.key);
      }

      if (row.hasText) expected.add(row.key);
    }
  }

  // dialogue rows carry renderable text and are merged by stable source key.
  function recordDialogue(definition, rows) {
    const structure = structuralKeys.get(definition.code) ?? new Set();
    for (const row of rows) {
      if (!structure.has(row.key)) {
        diagnostics.errors.push(`Dialogue key is not present in structure for ${definition.code}: ${row.key}`);
        continue;
      }

      let source;
      try {
        const cacheKey = renderCacheKey(row.ir);
        source = {
          ir: row.ir,
          text: cleanText(renderAuditText(row.ir)),
          cacheId: dialogueId(cacheKey),
        };
      } catch (error) {
        diagnostics.errors.push(`Failed to compile dialogue IR ${definition.code}/${row.key} at ${row.source}: ${error.message}`);
        continue;
      }

      mergeEntry(row.key, definition, source, {
        sourceKind: sourceKindFromSheet(row.sheet),
        sheet: row.sheet,
        row: row.row,
        column: row.column,
      });
    }
  }

  // one release entry can accumulate text from several languages.
  function mergeEntry(key, definition, dialogue, source) {
    let entry = entries.get(key);
    if (!entry) {
      entry = {
        key,
        sourceKind: source.sourceKind,
        sheet: source.sheet,
        row: source.row,
        column: source.column,
        japanese: "",
        english: "",
        german: "",
        french: "",
        simplifiedChinese: "",
        traditionalChinese: "",
        korean: "",
      };
      entries.set(key, entry);
    }

    const property = definition.property;
    if (entry[property] && entry[property].cacheId !== dialogue.cacheId) {
      diagnostics.duplicates.push({
        key,
        language: definition.code,
        previous: entry[property].text,
        next: dialogue.text,
        source,
      });
      return;
    }

    entry[property] = dialogue;
    getKeySet(emittedTextKeys, definition.code).add(key);
    diagnostics.languages[definition.code].textEntries++;
  }

  function dialogueId(cacheKey) {
    let id = dialogueIds.get(cacheKey);
    if (!id) {
      id = nextDialogueId++;
      dialogueIds.set(cacheKey, id);
    }

    return id;
  }
}

function getKeySet(map, language) {
  let set = map.get(language);
  if (!set) {
    set = new Set();
    map.set(language, set);
  }

  return set;
}

function sourceKindFromSheet(sheet) {
  if (sheet === "DefaultTalk") return "DefaultTalk";
  const first = String(sheet ?? "").split("/")[0];
  return first || "unknown";
}
