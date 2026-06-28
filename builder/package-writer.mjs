import path from "node:path";

import {
  dialogueComponentId,
  packageEntriesMagic,
  packageFormat,
  packageFormatVersion,
  packageLanguageDefinitions,
  packageMatchIndexMagic,
  packageTemplateMagic,
} from "./contract.mjs";
import { StringPool, writeShardMagic, writeU64Array } from "./binary.mjs";
import { buildMatchCandidates, encodeTargetTemplate, renderTargetTemplate } from "./dialogue-ir-compiler.mjs";
import { publicJsonBuffer, writeFileEnsuringDirectory } from "./utils.mjs";

// package root
// --------------------------------
export function buildPackage(packageSourceDirectory, records, { buildNumber, gameVersion, builtAt }) {
  const files = [];
  const writePackageFile = (archivePath, data) => {
    writeFileEnsuringDirectory(path.join(packageSourceDirectory, archivePath), data);
    files.push({ path: archivePath, data });
  };

  records.forEach((entry, index) => {
    entry.entryId = index;
  });

  writePackageFile(`${dialogueComponentId}/entries.bttbin`, buildEntriesShard(records));

  const languageDiagnostics = {};
  for (const definition of packageLanguageDefinitions) {
    const templateShard = buildTemplateShard(records, definition);
    writePackageFile(`${dialogueComponentId}/template/${definition.code}.bttbin`, templateShard.buffer);

    const matchShard = buildMatchShard(records, definition);
    writePackageFile(`${dialogueComponentId}/match/${definition.code}.bttbin`, matchShard.buffer);

    languageDiagnostics[definition.code] = {
      templateCount: templateShard.templateCount,
      exactCount: matchShard.exactCount,
      exactCandidateCount: matchShard.exactCandidateCount,
      patternCount: matchShard.patternCount,
      patternTokenCount: matchShard.patternTokenCount,
    };
  }

  const manifest = {
    format: packageFormat,
    formatVersion: packageFormatVersion,
    buildNumber,
    gameVersion,
    entryCount: records.length,
    builtAt,
    components: {
      [dialogueComponentId]: {},
    },
  };
  writePackageFile("manifest.json", publicJsonBuffer(manifest));

  return {
    files,
    diagnostics: {
      sourceDirectory: path.basename(packageSourceDirectory),
      languages: languageDiagnostics,
    },
  };
}

// entries shard
// --------------------------------
function buildEntriesShard(records) {
  const pool = new StringPool();
  const rows = [];
  for (const entry of records) {
    let presence = 0;
    packageLanguageDefinitions.forEach((definition, index) => {
      if (entry[definition.property]) presence |= 1 << index;
    });

    rows.push({
      key: pool.add(entry.key),
      sheet: pool.add(entry.sheet ?? ""),
      row: pool.add(entry.row ?? ""),
      column: pool.add(entry.column ?? ""),
      presence,
    });
  }

  const stringPool = pool.toBuffers();
  const headerSize = 72;
  const rowSize = 17;
  const stringOffsetsOffset = headerSize;
  const entriesOffset = stringOffsetsOffset + stringPool.offsets.length;
  const rowBytesLength = rows.length * rowSize;
  const stringBytesOffset = entriesOffset + rowBytesLength;
  const header = Buffer.alloc(headerSize);
  writeShardMagic(header, packageEntriesMagic);
  header.writeUInt16LE(1, 16);
  header.writeUInt16LE(headerSize, 18);
  header.writeUInt32LE(0, 20);
  header.writeUInt32LE(rows.length, 24);
  header.writeUInt32LE(pool.count, 28);
  header.writeUInt32LE(rowSize, 32);
  header.writeUInt32LE(0, 36);
  header.writeBigUInt64LE(BigInt(entriesOffset), 40);
  header.writeBigUInt64LE(BigInt(stringOffsetsOffset), 48);
  header.writeBigUInt64LE(BigInt(stringBytesOffset), 56);
  header.writeBigUInt64LE(BigInt(stringPool.bytes.length), 64);

  const rowBuffer = Buffer.alloc(rowBytesLength);
  rows.forEach((row, index) => {
    const offset = index * rowSize;
    rowBuffer.writeUInt32LE(row.key, offset);
    rowBuffer.writeUInt32LE(row.sheet, offset + 4);
    rowBuffer.writeUInt32LE(row.row, offset + 8);
    rowBuffer.writeUInt32LE(row.column, offset + 12);
    rowBuffer.writeUInt8(row.presence, offset + 16);
  });

  return Buffer.concat([header, stringPool.offsets, rowBuffer, stringPool.bytes]);
}

// target template shards
// --------------------------------
function buildTemplateShard(records, definition) {
  const chunks = [];
  const offsets = new BigUint64Array(records.length + 1);
  const templateCache = new Map();
  let totalBytes = 0;
  let templateCount = 0;
  for (let index = 0; index < records.length; index++) {
    offsets[index] = BigInt(totalBytes);
    const source = records[index][definition.property] ?? null;
    const cacheKey = source?.cacheId ?? "";
    let cached = templateCache.get(cacheKey);
    if (!cached) {
      const template = source ? renderTargetTemplate(source) : [];
      cached = {
        bytes: encodeTargetTemplate(template),
        hasTemplate: template.length > 0,
      };
      templateCache.set(cacheKey, cached);
    }
    if (cached.hasTemplate) templateCount++;
    const bytes = cached.bytes;
    chunks.push(bytes);
    totalBytes += bytes.length;
  }
  offsets[records.length] = BigInt(totalBytes);

  const offsetBuffer = writeU64Array(offsets);
  const textBytes = Buffer.concat(chunks);
  const headerSize = 56;
  const offsetsOffset = headerSize;
  const textBytesOffset = offsetsOffset + offsetBuffer.length;
  const header = Buffer.alloc(headerSize);
  writeShardMagic(header, packageTemplateMagic);
  header.writeUInt16LE(1, 16);
  header.writeUInt16LE(headerSize, 18);
  header.writeUInt32LE(0, 20);
  header.writeUInt32LE(records.length, 24);
  header.writeUInt32LE(templateCount, 28);
  header.writeBigUInt64LE(BigInt(offsetsOffset), 32);
  header.writeBigUInt64LE(BigInt(textBytesOffset), 40);
  header.writeBigUInt64LE(BigInt(textBytes.length), 48);

  return {
    buffer: Buffer.concat([header, offsetBuffer, textBytes]),
    templateCount,
  };
}

// match index shards
// --------------------------------
function buildMatchShard(records, definition) {
  const exactMap = new Map();
  const patterns = [];
  const candidateCache = new Map();
  const property = definition.property;

  for (const entry of records) {
    const source = entry[property] ?? null;
    if (!source) continue;

    const cacheKey = source.cacheId;
    let candidates = candidateCache.get(cacheKey);
    if (!candidates) {
      candidates = buildMatchCandidates(source);
      candidateCache.set(cacheKey, candidates);
    }
    const patternSeen = new Set();
    for (const candidate of candidates) {
      if (candidate.isExact) {
        let set = exactMap.get(candidate.normalisedSource);
        if (!set) {
          set = new Set();
          exactMap.set(candidate.normalisedSource, set);
        }

        set.add(entry.entryId);
      } else {
        const key = candidate.tokens.join("\u0000");
        if (patternSeen.has(key)) continue;
        patternSeen.add(key);
        patterns.push({
          entryId: entry.entryId,
          tokens: candidate.tokens,
        });
      }
    }
  }

  const pool = new StringPool();
  const exactRows = [...exactMap.entries()]
    .map(([text, ids]) => ({ text, ids, textBytes: Buffer.from(text, "utf8") }))
    .sort((left, right) => Buffer.compare(left.textBytes, right.textBytes))
    .map((row) => ({
      textId: pool.add(row.text),
      ids: [...row.ids].sort((left, right) => left - right),
    }));
  const exactCandidates = exactRows.flatMap((row) => row.ids);

  let candidateStart = 0;
  const exactRowBuffer = Buffer.alloc(exactRows.length * 12);
  exactRows.forEach((row, index) => {
    const offset = index * 12;
    exactRowBuffer.writeUInt32LE(row.textId, offset);
    exactRowBuffer.writeUInt32LE(candidateStart, offset + 4);
    exactRowBuffer.writeUInt32LE(row.ids.length, offset + 8);
    candidateStart += row.ids.length;
  });

  const exactCandidateBuffer = Buffer.alloc(exactCandidates.length * 4);
  exactCandidates.forEach((entryId, index) => {
    exactCandidateBuffer.writeUInt32LE(entryId, index * 4);
  });

  const patternTokenIds = [];
  const patternRowBuffer = Buffer.alloc(patterns.length * 16);
  patterns.forEach((pattern, index) => {
    const tokenStart = patternTokenIds.length;
    for (const token of pattern.tokens) {
      patternTokenIds.push(pool.add(token));
    }

    const offset = index * 16;
    patternRowBuffer.writeUInt32LE(pattern.entryId, offset);
    patternRowBuffer.writeUInt32LE(tokenStart, offset + 4);
    patternRowBuffer.writeUInt16LE(pattern.tokens.length, offset + 8);
    patternRowBuffer.writeUInt16LE(0, offset + 10);
    patternRowBuffer.writeUInt32LE(pattern.tokens.length > 0 ? pool.add(pattern.tokens[0]) : 0, offset + 12);
  });

  const patternTokenBuffer = Buffer.alloc(patternTokenIds.length * 4);
  patternTokenIds.forEach((tokenId, index) => {
    patternTokenBuffer.writeUInt32LE(tokenId, index * 4);
  });

  const stringPool = pool.toBuffers();
  const headerSize = 96;
  let offset = headerSize;
  const exactRowsOffset = offset;
  offset += exactRowBuffer.length;
  const exactCandidatesOffset = offset;
  offset += exactCandidateBuffer.length;
  const patternRowsOffset = offset;
  offset += patternRowBuffer.length;
  const patternTokensOffset = offset;
  offset += patternTokenBuffer.length;
  const stringOffsetsOffset = offset;
  offset += stringPool.offsets.length;
  const stringBytesOffset = offset;

  const header = Buffer.alloc(headerSize);
  writeShardMagic(header, packageMatchIndexMagic);
  header.writeUInt16LE(1, 16);
  header.writeUInt16LE(headerSize, 18);
  header.writeUInt32LE(0, 20);
  header.writeUInt32LE(records.length, 24);
  header.writeUInt32LE(exactRows.length, 28);
  header.writeUInt32LE(exactCandidates.length, 32);
  header.writeUInt32LE(patterns.length, 36);
  header.writeBigUInt64LE(BigInt(exactRowsOffset), 40);
  header.writeBigUInt64LE(BigInt(exactCandidatesOffset), 48);
  header.writeBigUInt64LE(BigInt(patternRowsOffset), 56);
  header.writeBigUInt64LE(BigInt(patternTokensOffset), 64);
  header.writeUInt32LE(patternTokenIds.length, 72);
  header.writeUInt32LE(pool.count, 76);
  header.writeBigUInt64LE(BigInt(stringOffsetsOffset), 80);
  header.writeBigUInt64LE(BigInt(stringBytesOffset), 88);

  return {
    buffer: Buffer.concat([
      header,
      exactRowBuffer,
      exactCandidateBuffer,
      patternRowBuffer,
      patternTokenBuffer,
      stringPool.offsets,
      stringPool.bytes,
    ]),
    exactCount: exactRows.length,
    exactCandidateCount: exactCandidates.length,
    patternCount: patterns.length,
    patternTokenCount: patternTokenIds.length,
  };
}
