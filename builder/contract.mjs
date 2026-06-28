// release layout
// --------------------------------
export const releaseManifestFileName = "latest.json";
export const packageSourceDirectoryName = "sources";
export const releasePackageDirectoryName = "packages";
export const packageFormatVersion = 1;
export const versionFormat = "btt-version";
export const packageFormat = "btt-package";
export const dialogueComponentId = "dialogue";

// source bundle contract
// --------------------------------
export const sourceBundleFormat = "btt-dialogue-source-bundle";
export const sourceBundleFormatVersion = 1;
export const sourceBundleKind = "dialogue-source";
export const sourceStructurePath = "structure.bttbin";
export const sourceDialoguePath = "dialogue.bttbin";
export const sourceBundleFiles = ["manifest.json", sourceStructurePath, sourceDialoguePath];
export const sourceScopes = ["DefaultTalk", "custom", "quest", "cut_scene"];
export const dialogueIrEncodingVersion = 2;

// binary shard contract
// --------------------------------
export const shardMagicBytes = 16;
export const sourceStructureMagic = "BTT-SRC-STRUCT";
export const sourceDialogueMagic = "BTT-SRC-DIALOGUE";
export const packageEntriesMagic = "BTT-PKG-ENTRIES";
export const packageTemplateMagic = "BTT-PKG-TEMPLATE";
export const packageMatchIndexMagic = "BTT-PKG-MATCHIDX";

// language model
// --------------------------------
export const packageLanguageDefinitions = [
  { code: "ja", property: "japanese" },
  { code: "en", property: "english" },
  { code: "de", property: "german" },
  { code: "fr", property: "french" },
  { code: "zh-Hans", property: "simplifiedChinese" },
  { code: "zh-Hant", property: "traditionalChinese" },
  { code: "ko", property: "korean" },
];

export const intlStructuralLanguages = ["ja", "en", "de", "fr"];
export const releaseKeyLanguages = ["ja", "en", "de", "fr"];
export const emptyPackageLanguages = ["zh-Hant", "ko"];
export const sourceLanguageDefinitions = packageLanguageDefinitions
  .filter((language) => !emptyPackageLanguages.includes(language.code));

export function languageProperty(code) {
  const definition = packageLanguageDefinitions.find((language) => language.code === code);
  if (!definition) throw new Error(`Unknown language code: ${code}`);
  return definition.property;
}

// template and matcher contract
// --------------------------------
export const templateExpansionDiagnosticLimit = 512;
export const wildcard = "\u001f";
export const templateOp = {
  text: 1,
  playerName: 2,
  gender: 3,
};
