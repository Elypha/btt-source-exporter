#!/usr/bin/env node
import { Command, InvalidArgumentError } from "commander";

import { buildDialogueRelease } from "./build-pipeline.mjs";
import { toolName } from "./utils.mjs";

const program = new Command()
  .name(toolName)
  .requiredOption("--source-root <path>", "directory containing language .bttsrc.tar.zst files")
  .requiredOption("--output <path>", "directory for release package output")
  .requiredOption("--build-number <n>", "positive release build number", positiveInteger)
  .option("--game-version <version>", "override source bundle gameVersion")
  .option("--limit <n>", "debug limit for emitted release entries", nonNegativeInteger)
  .option("--diagnose-template-expansion", "record template control-flow expansion diagnostics");

program.parse();

try {
  const result = buildDialogueRelease(program.opts());
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  process.exitCode = 1;
  console.error(error.message);
}

function positiveInteger(value) {
  const number = Number(value);
  if (!Number.isInteger(number) || number <= 0) {
    throw new InvalidArgumentError("must be a positive integer");
  }

  return number;
}

function nonNegativeInteger(value) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < 0) {
    throw new InvalidArgumentError("must be a non-negative integer");
  }

  return number;
}
