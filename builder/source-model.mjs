import fs from "node:fs";

const sourceModelUrl = new URL("../dialogue-sources.json", import.meta.url);

export function readDialogueSourceModel() {
  const model = JSON.parse(fs.readFileSync(sourceModelUrl, "utf8").replace(/^\uFEFF/, ""));
  if (model.version !== 1) {
    throw new Error(`Unsupported dialogue source model version: ${model.version}`);
  }
  if (!Array.isArray(model.sources) || model.sources.length === 0) {
    throw new Error("Dialogue source model must contain sources.");
  }

  const sourceScopes = [];
  const seen = new Set();
  for (const source of model.sources) {
    let scope = "";
    if (source.kind === "standaloneTalk") {
      scope = source.sheet;
      validateTextColumns(scope, source.textColumns);
    } else if (source.kind === "eventFolder") {
      scope = source.folder;
    }

    if (!scope || typeof scope !== "string") {
      throw new Error(`Invalid dialogue source model source: ${JSON.stringify(source)}`);
    }
    if (seen.has(scope)) throw new Error(`Duplicate dialogue source scope: ${scope}`);
    seen.add(scope);
    sourceScopes.push(scope);
  }

  return { sourceScopes };
}

function validateTextColumns(scope, textColumns) {
  if (!Array.isArray(textColumns) || textColumns.length === 0) {
    throw new Error(`Standalone talk source ${scope} must define textColumns.`);
  }

  const seen = new Set();
  for (const column of textColumns) {
    if (!column || typeof column !== "string") {
      throw new Error(`Standalone talk source ${scope} contains an invalid text column.`);
    }
    if (seen.has(column)) {
      throw new Error(`Standalone talk source ${scope} contains duplicate text column: ${column}`);
    }
    seen.add(column);
  }
}
