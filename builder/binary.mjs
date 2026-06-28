import { shardMagicBytes } from "./contract.mjs";

// string pool
// --------------------------------
export class StringPool {
  constructor() {
    this.ids = new Map();
    this.values = [];
  }

  get count() {
    return this.values.length;
  }

  add(value) {
    value = String(value ?? "");
    const existing = this.ids.get(value);
    if (existing != null) return existing;

    const id = this.values.length;
    this.values.push(value);
    this.ids.set(value, id);
    return id;
  }

  toBuffers() {
    const chunks = [];
    const offsets = new BigUint64Array(this.values.length + 1);
    let totalBytes = 0;
    for (let index = 0; index < this.values.length; index++) {
      offsets[index] = BigInt(totalBytes);
      const bytes = Buffer.from(this.values[index], "utf8");
      chunks.push(bytes);
      totalBytes += bytes.length;
    }
    offsets[this.values.length] = BigInt(totalBytes);

    return {
      offsets: writeU64Array(offsets),
      bytes: Buffer.concat(chunks),
    };
  }
}

// dialogue IR reader
// --------------------------------
export function createBinaryReader(buffer, start, end, label) {
  return {
    buffer,
    offset: start,
    end,
    label,
    readU8() {
      this.assertAvailable(1);
      return this.buffer[this.offset++];
    },
    readU32() {
      this.assertAvailable(4);
      const value = this.buffer.readUInt32LE(this.offset);
      this.offset += 4;
      return value;
    },
    assertAvailable(length) {
      if (this.offset + length > this.end) {
        throw new Error(`${this.label}: truncated dialogue IR node at byte ${this.offset}`);
      }
    },
  };
}

// shard headers
// --------------------------------
export function validateShardHeader(buffer, magic, schemaVersion, headerSize, label) {
  if (!Buffer.isBuffer(buffer)) throw new Error(`${label}: missing shard buffer`);
  if (buffer.length < headerSize) throw new Error(`${label}: shard is shorter than header size ${headerSize}`);
  const actualMagic = readShardMagic(buffer);
  if (actualMagic !== magic) throw new Error(`${label}: magic must be ${magic}, found ${actualMagic}`);
  const actualSchemaVersion = buffer.readUInt16LE(shardMagicBytes);
  if (actualSchemaVersion !== schemaVersion) {
    throw new Error(`${label}: schemaVersion must be ${schemaVersion}, found ${actualSchemaVersion}`);
  }

  const actualHeaderSize = buffer.readUInt16LE(shardMagicBytes + 2);
  if (actualHeaderSize !== headerSize) throw new Error(`${label}: headerSize must be ${headerSize}, found ${actualHeaderSize}`);
}

function readShardMagic(buffer) {
  return buffer.subarray(0, shardMagicBytes).toString("ascii").replace(/\0+$/, "");
}

export function writeShardMagic(buffer, magic) {
  const byteLength = Buffer.byteLength(magic, "ascii");
  if (byteLength > shardMagicBytes) throw new Error(`shard magic is longer than ${shardMagicBytes} bytes: ${magic}`);
  buffer.write(magic, 0, byteLength, "ascii");
}

// string pool readers
// --------------------------------
export function readStringPool(buffer, count, offsetsOffset, bytesOffset, bytesLength, label) {
  const offsets = readU64Offsets(buffer, count + 1, offsetsOffset, label);
  if (offsets[offsets.length - 1] !== bytesLength) {
    throw new Error(`${label}: final string offset ${offsets[offsets.length - 1]} must equal string byte length ${bytesLength}`);
  }

  const strings = [];
  const decoder = new TextDecoder("utf-8", { fatal: true });
  for (let index = 0; index < count; index++) {
    const start = bytesOffset + offsets[index];
    const end = bytesOffset + offsets[index + 1];
    try {
      strings.push(decoder.decode(buffer.subarray(start, end)));
    } catch (error) {
      throw new Error(`${label}: invalid UTF-8 string ${index}: ${error.message}`);
    }
  }

  return strings;
}

export function readU64Offsets(buffer, count, offset, label) {
  const offsets = [];
  for (let index = 0; index < count; index++) {
    offsets.push(readU64Number(buffer, offset + index * 8, `${label}[${index}]`));
    if (index === 0 && offsets[index] !== 0) throw new Error(`${label}: first offset must be 0`);
    if (index > 0 && offsets[index] < offsets[index - 1]) throw new Error(`${label}: offsets must be monotonic at index ${index}`);
  }

  return offsets;
}

export function stringById(strings, id, label) {
  if (!Number.isInteger(id) || id < 0 || id >= strings.length) {
    throw new Error(`${label}: string id ${id} is out of range 0..${strings.length - 1}`);
  }

  return strings[id];
}

// primitive readers
// --------------------------------
export function readU32(buffer, offset, label) {
  assertRange(buffer, offset, 4, `${label}:u32@${offset}`);
  return buffer.readUInt32LE(offset);
}

export function readU64Number(buffer, offset, label) {
  assertRange(buffer, offset, 8, `${label}:u64@${offset}`);
  const value = buffer.readBigUInt64LE(offset);
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(`${label}: u64 value exceeds Number.MAX_SAFE_INTEGER`);
  }

  return Number(value);
}

export function assertRange(buffer, offset, length, label) {
  if (!Number.isInteger(offset) || offset < 0) throw new Error(`${label}: invalid offset ${offset}`);
  if (!Number.isInteger(length) || length < 0) throw new Error(`${label}: invalid length ${length}`);
  if (offset + length > buffer.length) {
    throw new Error(`${label}: range ${offset}..${offset + length} exceeds buffer length ${buffer.length}`);
  }
}

export function assertAligned(offset, label) {
  if (offset % 8 !== 0) throw new Error(`${label}: offset ${offset} is not 8-byte aligned`);
}

// primitive writers
// --------------------------------
export function writeU8(value) {
  const buffer = Buffer.alloc(1);
  buffer.writeUInt8(value);
  return buffer;
}

export function writeU32(value) {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32LE(value);
  return buffer;
}

export function writeU64Array(values) {
  const buffer = Buffer.alloc(values.length * 8);
  for (let index = 0; index < values.length; index++) {
    buffer.writeBigUInt64LE(BigInt(values[index]), index * 8);
  }

  return buffer;
}
