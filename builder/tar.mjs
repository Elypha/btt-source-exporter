export function parseTarArchive(buffer, archivePath) {
  const entries = new Map();
  let offset = 0;

  while (offset < buffer.length) {
    const header = buffer.subarray(offset, offset + 512);
    if (header.length < 512) throw new Error(`truncated tar header at byte ${offset}`);
    offset += 512;

    if (isZeroBlock(header)) {
      const next = buffer.subarray(offset, offset + 512);
      if (next.length >= 512 && isZeroBlock(next)) offset += 512;
      break;
    }

    validateTarChecksum(header, archivePath, offset - 512);

    const name = readTarString(header, 0, 100);
    const prefix = readTarString(header, 345, 155);
    const fullName = prefix ? `${prefix}/${name}` : name;
    const typeFlag = header[156];
    const size = readTarOctalField(header, 124, 12, `${archivePath}:${fullName}:size`);
    if (!isSafeArchivePath(fullName)) throw new Error(`unsafe tar path: ${fullName}`);
    if (typeFlag !== 0 && typeFlag !== 0x30) throw new Error(`unsupported tar entry type ${String.fromCharCode(typeFlag)} for ${fullName}`);
    if (entries.has(fullName)) throw new Error(`duplicate tar entry: ${fullName}`);

    const data = buffer.subarray(offset, offset + size);
    if (data.length !== size) throw new Error(`truncated tar entry: ${fullName}`);
    entries.set(fullName, Buffer.from(data));
    offset += size;
    offset += (512 - (size % 512)) % 512;
  }

  if (offset > buffer.length) throw new Error("tar offset exceeded archive length");
  return entries;
}

export function createTarBuffer(files) {
  const chunks = [];
  for (const file of [...files].sort((left, right) => left.path.localeCompare(right.path, "en"))) {
    if (!isSafeArchivePath(file.path)) throw new Error(`unsafe tar path: ${file.path}`);
    chunks.push(tarHeader(file.path, file.data.length));
    chunks.push(file.data);
    const padding = (512 - (file.data.length % 512)) % 512;
    if (padding > 0) chunks.push(Buffer.alloc(padding));
  }

  chunks.push(Buffer.alloc(1024));
  return Buffer.concat(chunks);
}

function isZeroBlock(buffer) {
  for (const byte of buffer) {
    if (byte !== 0) return false;
  }

  return true;
}

function validateTarChecksum(header, archivePath, offset) {
  const stored = readTarOctalField(header, 148, 8, `${archivePath}:checksum`);
  let sum = 0;
  for (let index = 0; index < header.length; index++) {
    sum += index >= 148 && index < 156 ? 0x20 : header[index];
  }

  if (stored !== sum) {
    throw new Error(`tar checksum mismatch at byte ${offset}: stored ${stored}, computed ${sum}`);
  }
}

function readTarString(buffer, offset, length) {
  const slice = buffer.subarray(offset, offset + length);
  const end = slice.indexOf(0);
  return slice.subarray(0, end < 0 ? slice.length : end).toString("utf8");
}

function readTarOctalField(buffer, offset, length, label) {
  const text = buffer.subarray(offset, offset + length)
    .toString("ascii")
    .replace(/\0.*$/s, "")
    .trim();
  if (!text) return 0;
  if (!/^[0-7]+$/.test(text)) throw new Error(`${label}: invalid tar octal value ${JSON.stringify(text)}`);
  return Number.parseInt(text, 8);
}

function isSafeArchivePath(value) {
  if (!value || value.includes("\\") || value.startsWith("/")) return false;
  const parts = value.split("/");
  return parts.every((part) => part && part !== "." && part !== "..");
}

function tarHeader(name, size) {
  if (Buffer.byteLength(name, "utf8") > 100) {
    throw new Error(`tar path is too long for the release writer: ${name}`);
  }

  const header = Buffer.alloc(512);
  header.write(name, 0, 100, "utf8");
  writeTarOctal(header, 100, 8, 0o644);
  writeTarOctal(header, 108, 8, 0);
  writeTarOctal(header, 116, 8, 0);
  writeTarOctal(header, 124, 12, size);
  writeTarOctal(header, 136, 12, 0);
  header.fill(0x20, 148, 156);
  header[156] = 0x30;
  header.write("ustar\0", 257, 6, "ascii");
  header.write("00", 263, 2, "ascii");
  header.write("btt", 265, 3, "ascii");
  header.write("btt", 297, 3, "ascii");

  let checksum = 0;
  for (const byte of header) checksum += byte;
  writeTarOctal(header, 148, 8, checksum);
  return header;
}

function writeTarOctal(buffer, offset, length, value) {
  const text = value.toString(8).padStart(length - 1, "0");
  buffer.write(text.slice(-length + 1), offset, length - 1, "ascii");
  buffer[offset + length - 1] = 0;
}
