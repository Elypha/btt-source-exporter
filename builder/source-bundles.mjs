import fs from "node:fs";
import zlib from "node:zlib";
import { z } from "zod";

import {
  dialogueIrEncodingVersion,
  sourceBundleFiles,
  sourceDialogueMagic,
  sourceDialoguePath,
  sourceBundleFormat,
  sourceBundleFormatVersion,
  sourceBundleKind,
  sourceStructureMagic,
  sourceStructurePath,
  sourceScopes,
} from "./contract.mjs";
import { decodeDialogueIr } from "./dialogue-ir.mjs";
import {
  assertAligned,
  assertRange,
  readStringPool,
  readU32,
  readU64Number,
  readU64Offsets,
  stringById,
  validateShardHeader,
} from "./binary.mjs";
import { parseTarArchive } from "./tar.mjs";
import { arraysEqual, cleanText } from "./utils.mjs";

// source bundle archive
// --------------------------------
export function readSourceBundle(definition, file, label = file) {
  const errors = [];
  const archiveBuffer = fs.readFileSync(file);

  let tarBuffer;
  try {
    tarBuffer = zlib.zstdDecompressSync(archiveBuffer);
  } catch (error) {
    return { errors: [`Failed to decompress source bundle for ${definition.code}: ${label}: ${error.message}`] };
  }

  let archive;
  try {
    archive = parseTarArchive(tarBuffer, label);
  } catch (error) {
    return { errors: [`Invalid source bundle archive for ${definition.code}: ${label}: ${error.message}`] };
  }

  const actualFiles = [...archive.keys()].sort((left, right) => left.localeCompare(right, "en"));
  const expectedFiles = [...sourceBundleFiles].sort((left, right) => left.localeCompare(right, "en"));
  if (!arraysEqual(actualFiles, expectedFiles)) {
    return {
      errors: [
        `Source bundle files differ for ${definition.code}: expected ${expectedFiles.join(", ")}, found ${actualFiles.join(", ")}.`,
      ],
    };
  }

  let manifest;
  try {
    manifest = JSON.parse(archive.get("manifest.json").toString("utf8").replace(/^\uFEFF/, ""));
  } catch (error) {
    return { errors: [`Invalid source bundle manifest for ${definition.code}: ${label}: ${error.message}`] };
  }

  const manifestResult = validateSourceBundleManifest(definition, label, manifest);
  errors.push(...manifestResult.errors);
  if (!manifestResult.manifest) return { errors };

  let structureRows;
  let dialogueRows;
  try {
    structureRows = readStructureShard(definition, archive.get(sourceStructurePath), `${label}:${sourceStructurePath}`);
    dialogueRows = readDialogueShard(definition, archive.get(sourceDialoguePath), `${label}:${sourceDialoguePath}`);
  } catch (error) {
    return { errors: [`Failed to read source bundle for ${definition.code}: ${error.message}`] };
  }

  if (structureRows.length !== manifestResult.manifest.structureRecords) {
    errors.push(`Structure record count mismatch for ${definition.code}: manifest ${manifestResult.manifest.structureRecords}, shard ${structureRows.length}.`);
  }

  if (dialogueRows.length !== manifestResult.manifest.dialogueRecords) {
    errors.push(`Dialogue record count mismatch for ${definition.code}: manifest ${manifestResult.manifest.dialogueRecords}, shard ${dialogueRows.length}.`);
  }

  return {
    manifest: manifestResult.manifest,
    structureRows,
    dialogueRows,
    errors,
  };
}

// manifest
// --------------------------------
const nonEmptyString = z.string().min(1);
const nonNegativeInteger = z.number().int().nonnegative();
const positiveInteger = z.number().int().positive();

const sourceBundleManifestSchema = z.strictObject({
  format: z.literal(sourceBundleFormat),
  formatVersion: z.literal(sourceBundleFormatVersion),
  kind: z.literal(sourceBundleKind),
  language: nonEmptyString,
  gameVersion: nonEmptyString,
  structureSchemaVersion: z.literal(1),
  dialogueSchemaVersion: z.literal(1),
  dialogueIrEncodingVersion: z.literal(dialogueIrEncodingVersion),
  scopeMode: z.literal("default-scopes"),
  sourceScopes: z.array(nonEmptyString)
    .refine((value) => arraysEqual(value, sourceScopes), { message: `must be ${sourceScopes.join(", ")}` }),
  sheets: z.array(nonEmptyString).nonempty(),
  sheetCount: positiveInteger,
  structureRecords: positiveInteger,
  dialogueRecords: positiveInteger,
  emptyTextRecords: nonNegativeInteger,
  skippedEmptyKeys: nonNegativeInteger,
}).superRefine((manifest, context) => {
  if (manifest.sheetCount !== manifest.sheets.length) {
    context.addIssue({
      code: "custom",
      path: ["sheetCount"],
      message: "must equal sheets.length",
    });
  }

  if (manifest.emptyTextRecords !== manifest.structureRecords - manifest.dialogueRecords) {
    context.addIssue({
      code: "custom",
      path: ["emptyTextRecords"],
      message: "must equal structureRecords - dialogueRecords",
    });
  }
});

function validateSourceBundleManifest(definition, file, manifest) {
  const parsed = parseSchema(sourceBundleManifestSchema, manifest, `Invalid source bundle manifest for ${definition.code}`);
  const errors = [...parsed.errors];
  if (!parsed.value) {
    errors.push(`Source bundle rejected for ${definition.code}: ${file}`);
    return { manifest: null, errors };
  }

  const value = parsed.value;
  if (value.language !== definition.code) {
    errors.push(`Invalid source bundle manifest for ${definition.code}: language must be ${definition.code}.`);
  }
  if (errors.length > 0) errors.push(`Source bundle rejected for ${definition.code}: ${file}`);

  return { manifest: errors.length > 0 ? null : value, errors };
}

function parseSchema(schema, value, label) {
  const result = schema.safeParse(value);
  if (result.success) return { value: result.data, errors: [] };
  return { value: null, errors: formatZodIssues(result.error, label) };
}

function formatZodIssues(error, label) {
  return error.issues.map((issue) => {
    const path = issue.path.length > 0 ? `.${issue.path.join(".")}` : "";
    return `${label}${path}: ${issue.message}`;
  });
}

// source shard readers
// --------------------------------
function readStructureShard(definition, buffer, label) {
  validateShardHeader(buffer, sourceStructureMagic, 1, 72, label);
  const recordCount = readU32(buffer, 24, label);
  const stringCount = readU32(buffer, 28, label);
  const rowSize = readU32(buffer, 32, label);
  const rowsOffset = readU64Number(buffer, 40, label);
  const stringOffsetsOffset = readU64Number(buffer, 48, label);
  const stringBytesOffset = readU64Number(buffer, 56, label);
  const stringBytesLength = readU64Number(buffer, 64, label);

  if (rowSize !== 20) throw new Error(`${label}: structure rowSize must be 20, found ${rowSize}`);
  assertAligned(rowsOffset, `${label}:rowsOffset`);
  assertAligned(stringOffsetsOffset, `${label}:stringOffsetsOffset`);
  assertAligned(stringBytesOffset, `${label}:stringBytesOffset`);
  assertRange(buffer, rowsOffset, recordCount * rowSize, `${label}:rows`);
  assertRange(buffer, stringOffsetsOffset, (stringCount + 1) * 8, `${label}:stringOffsets`);
  assertRange(buffer, stringBytesOffset, stringBytesLength, `${label}:stringBytes`);
  if (stringOffsetsOffset < rowsOffset + recordCount * rowSize) throw new Error(`${label}: string offsets overlap structure rows`);
  if (stringBytesOffset < stringOffsetsOffset + (stringCount + 1) * 8) throw new Error(`${label}: string bytes overlap string offsets`);

  const strings = readStringPool(buffer, stringCount, stringOffsetsOffset, stringBytesOffset, stringBytesLength, `${label}:strings`);
  const rows = [];
  for (let index = 0; index < recordCount; index++) {
    const offset = rowsOffset + index * rowSize;
    const key = cleanText(stringById(strings, readU32(buffer, offset, label), `${label}:row${index}:key`));
    if (!key) throw new Error(`${label}:row${index}: empty structure key`);

    const sheet = stringById(strings, readU32(buffer, offset + 4, label), `${label}:row${index}:sheet`);
    const row = stringById(strings, readU32(buffer, offset + 8, label), `${label}:row${index}:row`);
    const column = stringById(strings, readU32(buffer, offset + 12, label), `${label}:row${index}:column`);
    const flags = readU32(buffer, offset + 16, label);
    if ((flags & ~1) !== 0) throw new Error(`${label}:row${index}: unsupported structure flags ${flags}`);

    rows.push({
      key,
      sheet,
      row,
      column,
      hasText: (flags & 1) !== 0,
      source: `${label}#${index}`,
    });
  }

  return rows;
}

function readDialogueShard(definition, buffer, label) {
  validateShardHeader(buffer, sourceDialogueMagic, 1, 88, label);
  const recordCount = readU32(buffer, 24, label);
  const stringCount = readU32(buffer, 28, label);
  const rowSize = readU32(buffer, 32, label);
  const rowsOffset = readU64Number(buffer, 40, label);
  const irOffsetsOffset = readU64Number(buffer, 48, label);
  const irBytesOffset = readU64Number(buffer, 56, label);
  const stringOffsetsOffset = readU64Number(buffer, 64, label);
  const stringBytesOffset = readU64Number(buffer, 72, label);
  const stringBytesLength = readU64Number(buffer, 80, label);

  if (rowSize !== 16) throw new Error(`${label}: dialogue rowSize must be 16, found ${rowSize}`);
  assertAligned(rowsOffset, `${label}:rowsOffset`);
  assertAligned(irOffsetsOffset, `${label}:irOffsetsOffset`);
  assertAligned(irBytesOffset, `${label}:irBytesOffset`);
  assertAligned(stringOffsetsOffset, `${label}:stringOffsetsOffset`);
  assertAligned(stringBytesOffset, `${label}:stringBytesOffset`);
  assertRange(buffer, rowsOffset, recordCount * rowSize, `${label}:rows`);
  assertRange(buffer, irOffsetsOffset, (recordCount + 1) * 8, `${label}:irOffsets`);
  assertRange(buffer, stringOffsetsOffset, (stringCount + 1) * 8, `${label}:stringOffsets`);
  assertRange(buffer, stringBytesOffset, stringBytesLength, `${label}:stringBytes`);
  if (irOffsetsOffset < rowsOffset + recordCount * rowSize) throw new Error(`${label}: IR offsets overlap dialogue rows`);
  if (irBytesOffset < irOffsetsOffset + (recordCount + 1) * 8) throw new Error(`${label}: IR bytes overlap IR offsets`);
  if (stringOffsetsOffset < irBytesOffset) throw new Error(`${label}: string offsets precede IR bytes`);
  if (stringBytesOffset < stringOffsetsOffset + (stringCount + 1) * 8) throw new Error(`${label}: string bytes overlap string offsets`);

  const irOffsets = readU64Offsets(buffer, recordCount + 1, irOffsetsOffset, `${label}:irOffsets`);
  const irBytesLength = irOffsets[irOffsets.length - 1];
  assertRange(buffer, irBytesOffset, irBytesLength, `${label}:irBytes`);
  if (irBytesOffset + irBytesLength > stringOffsetsOffset) throw new Error(`${label}: IR bytes overlap string offsets`);

  const strings = readStringPool(buffer, stringCount, stringOffsetsOffset, stringBytesOffset, stringBytesLength, `${label}:strings`);
  const rows = [];
  for (let index = 0; index < recordCount; index++) {
    const offset = rowsOffset + index * rowSize;
    const key = cleanText(stringById(strings, readU32(buffer, offset, label), `${label}:row${index}:key`));
    if (!key) throw new Error(`${label}:row${index}: empty dialogue key`);

    const sheet = stringById(strings, readU32(buffer, offset + 4, label), `${label}:row${index}:sheet`);
    const row = stringById(strings, readU32(buffer, offset + 8, label), `${label}:row${index}:row`);
    const column = stringById(strings, readU32(buffer, offset + 12, label), `${label}:row${index}:column`);
    const irStart = irBytesOffset + irOffsets[index];
    const irEnd = irBytesOffset + irOffsets[index + 1];
    const ir = decodeDialogueIr(buffer, irStart, irEnd, strings, `${label}#${index}`);

    rows.push({
      key,
      sheet,
      row,
      column,
      ir,
      source: `${label}#${index}`,
    });
  }

  return rows;
}
