import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

import {
  packageFormatVersion,
  packageSourceDirectoryName,
  releaseManifestFileName,
  releasePackageDirectoryName,
  versionFormat,
} from "./contract.mjs";
import { buildPackage } from "./package-writer.mjs";
import { createTarBuffer } from "./tar.mjs";
import { packageArchiveFileName, sha256File, writePublicJson } from "./utils.mjs";

// release artifacts
// --------------------------------
export function writeReleaseArtifacts({ outputDir, records, buildNumber, gameVersion, builtAt, diagnostics }) {
  fs.mkdirSync(outputDir, { recursive: true });

  const packageSourceDir = path.join(outputDir, packageSourceDirectoryName);
  const packageInfo = buildPackage(packageSourceDir, records, { buildNumber, gameVersion, builtAt });
  const archiveFileName = packageArchiveFileName(gameVersion, buildNumber);
  const archiveRelativePath = `${releasePackageDirectoryName}/${archiveFileName}`;
  const archivePath = path.join(outputDir, archiveRelativePath);
  const tarBuffer = createTarBuffer(packageInfo.files);
  const archiveBuffer = zlib.zstdCompressSync(tarBuffer, {
    params: {
      [zlib.constants.ZSTD_c_compressionLevel]: 10,
    },
  });
  fs.mkdirSync(path.dirname(archivePath), { recursive: true });
  fs.writeFileSync(archivePath, archiveBuffer);

  const archiveSha256 = sha256File(archivePath);
  const externalManifest = {
    format: versionFormat,
    formatVersion: packageFormatVersion,
    buildNumber,
    gameVersion,
    entryCount: records.length,
    builtAt,
    package: {
      path: archiveRelativePath,
      sha256: archiveSha256,
    },
  };

  writePublicJson(path.join(outputDir, releaseManifestFileName), externalManifest);
  writeDiagnostics(outputDir, diagnostics, {
    formatVersion: packageFormatVersion,
    entryCount: records.length,
    downloadPackagePath: archiveRelativePath,
    downloadPackageSha256: archiveSha256,
    package: packageInfo.diagnostics,
  });

  return {
    outputDir: path.relative(process.cwd(), outputDir) || ".",
    buildNumber,
    gameVersion,
    entryCount: records.length,
    downloadPackagePath: archiveRelativePath,
    downloadPackageSha256: archiveSha256,
    warnings: diagnostics.warnings,
  };
}

// local audit output
// --------------------------------
export function writeDiagnostics(directory, diagnostics, output = null) {
  fs.mkdirSync(directory, { recursive: true });
  if (output) diagnostics.output = output;
  writePublicJson(path.join(directory, "build-diagnostics.json"), diagnostics);
}
