import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";

export const toolName = "btt-package-builder";
export const logPrefix = `[${toolName}]`;

export function packageArchiveFileName(gameVersion, buildNumber) {
  if (!/^[0-9A-Za-z.-]+$/.test(gameVersion)) {
    throw new Error(`gameVersion is not safe for package file names: ${gameVersion}`);
  }

  return `${gameVersion}-build-${buildNumber}.tar.zst`;
}

export function arraysEqual(left, right) {
  if (!Array.isArray(left) || !Array.isArray(right)) return false;
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) return false;
  }

  return true;
}
export function normaliseRenderedText(text) {
  return normaliseRenderedFragment(text)
    // alphabetic writing systems have dynamic line breaks added at runtime
    // character-based languages (ja, zh) doesn't have the issue
    .replace(/\n/g, " ")
    .replace(/[ \t]+/g, " ")
    .trim();
}

export function normaliseRenderedFragment(text) {
  return String(text ?? "")
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .replace(/\u00a0/g, " ");
}

export function stringIsEmpty(value) {
  return value == null || String(value).trim() === "";
}

export function writeFileEnsuringDirectory(file, buffer) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, buffer);
}
export function cleanText(value) {
  if (value == null) return "";
  const text = String(value)
    .replace(/\r\n/g, "\n")
    .replace(/\r/g, "\n")
    .trim();
  return text === "0" ? "" : text;
}
export function writePublicJson(file, value) {
  writeFileEnsuringDirectory(file, publicJsonBuffer(value));
}

export function publicJsonBuffer(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}
export function sha256File(file) {
  const hash = crypto.createHash("sha256");
  const stream = fs.readFileSync(file);
  hash.update(stream);
  return hash.digest("hex");
}

export function readTextIfExists(file) {
  if (!fs.existsSync(file)) return null;
  return fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "").trim();
}
