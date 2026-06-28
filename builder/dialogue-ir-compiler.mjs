import { packageLanguageDefinitions, templateExpansionDiagnosticLimit, templateOp, wildcard } from "./contract.mjs";
import { writeU8, writeU32 } from "./binary.mjs";
import { normaliseRenderedFragment, normaliseRenderedText } from "./utils.mjs";

// audit and cache text
// --------------------------------
export function renderAuditText(ir) {
  switch (ir?.kind) {
    case "sequence":
      return ir.items.map(renderAuditText).join("");
    case "text":
      return ir.value ?? "";
    case "placeholder":
      return renderPlaceholderText(ir.value);
    case "parameter":
      return renderParameterText(ir);
    case "playerName":
      return "{object:1}";
    case "if":
      return `${renderAuditText(ir.then)}${renderAuditText(ir.else)}`;
    case "switch":
      return ir.cases.map(renderAuditText).join("");
    default:
      throw new Error(`unsupported dialogue IR node: ${ir?.kind}`);
  }
}

export function renderCacheKey(ir) {
  switch (ir?.kind) {
    case "sequence":
      return `\u001es${ir.items.map(renderCacheKey).join("")}\u001f`;
    case "text":
      return `\u001et${String(ir.value ?? "").length}:${ir.value ?? ""}`;
    case "placeholder":
      return `\u001ep${ir.value}`;
    case "parameter":
      return `\u001ea${ir.parameter}:${ir.index}`;
    case "playerName":
      return "\u001en";
    case "if":
      return `\u001ei${ir.condition}\u001f${renderCacheKey(ir.then)}${renderCacheKey(ir.else)}`;
    case "switch":
      return `\u001ew${ir.condition}\u001f${ir.cases.map(renderCacheKey).join("")}\u001f`;
    default:
      throw new Error(`unsupported dialogue IR node: ${ir?.kind}`);
  }
}

function renderParameterText(parameter) {
  switch (parameter.parameter) {
    case "object":
      return `{object:${parameter.index}}`;
    case "integer":
    case "string":
      return "{value}";
    case "player":
      return `PlayerParameter(${parameter.index})`;
    default:
      throw new Error(`unsupported dialogue IR parameter kind: ${parameter.parameter}`);
  }
}

function renderPlaceholderText(kind) {
  switch (kind) {
    case "sheet":
      return "{sheet}";
    case "icon":
      return "{icon}";
    case "time":
      return "{time}";
    case "softHyphen":
      return "";
    case "value":
      return "{value}";
    default:
      throw new Error(`unsupported dialogue IR placeholder kind: ${kind}`);
  }
}

// match candidates
// --------------------------------
export function buildMatchCandidates(source) {
  const candidates = [];
  for (const variant of renderSourceVariants(source.ir)) {
    const normalised = normaliseRenderedText(variant);
    if (!normalised) continue;
    const tokens = normalised
      .split(wildcard)
      .map((token) => token.trim())
      .filter(Boolean);
    if (tokens.length === 0) continue;
    candidates.push({
      normalisedSource: normalised,
      tokens,
      isExact: tokens.length === 1 && tokens[0] === normalised,
    });
  }

  return candidates;
}

function renderSourceVariants(ir) {
  switch (ir?.kind) {
    case "sequence":
      return combineStringVariants(ir.items, renderSourceVariants);
    case "text":
      return [ir.value ?? ""];
    case "placeholder":
      return [wildcard];
    case "parameter":
      return [ir.parameter === "player" ? `PlayerParameter(${ir.index})` : wildcard];
    case "playerName":
      return [wildcard];
    case "if":
      return deduplicateAndLimit([...renderSourceVariants(ir.then), ...renderSourceVariants(ir.else)]);
    case "switch":
      return ir.cases.length > 0
        ? deduplicateAndLimit(ir.cases.flatMap(renderSourceVariants))
        : [wildcard];
    default:
      throw new Error(`unsupported dialogue IR node: ${ir?.kind}`);
  }
}

function combineStringVariants(items, render) {
  let variants = [""];
  for (const item of items) {
    const itemVariants = render(item);
    const next = [];
    for (const prefix of variants) {
      for (const suffix of itemVariants) {
        next.push(prefix + suffix);
      }
    }

    variants = deduplicateAndLimit(next);
  }

  return variants;
}

function deduplicateAndLimit(inputs) {
  const output = [];
  const seen = new Set();
  for (const input of inputs) {
    if (seen.has(input)) continue;
    seen.add(input);
    output.push(input);
    if (output.length >= templateExpansionDiagnosticLimit) break;
  }

  return output;
}

// target templates
// --------------------------------
export function renderTargetTemplate(source, trim = true) {
  const ir = source?.ir ?? source;
  const parts = compileTargetParts(ir);
  return simplifyTemplateParts(trim ? trimTemplateParts(parts) : parts);
}

function compileTargetParts(ir) {
  switch (ir?.kind) {
    case "sequence":
      return simplifyTemplateParts(ir.items.flatMap(compileTargetParts));
    case "text":
      return textParts(normaliseRenderedFragment(ir.value ?? ""));
    case "placeholder":
      return textParts(renderPlaceholderText(ir.value));
    case "parameter":
      return compileParameterParts(ir);
    case "playerName":
      return [{ kind: "playerName" }];
    case "if":
      if (!isRuntimeGenderCondition(ir.condition)) {
        return compileTargetParts(ir.then);
      }

      return [{
        kind: "gender",
        female: renderTargetTemplate(ir.then, false),
        male: renderTargetTemplate(ir.else, false),
      }];
    case "switch":
      return ir.cases.length > 0
        ? compileTargetParts(ir.cases[0])
        : textParts(renderPlaceholderText("value"));
    default:
      throw new Error(`unsupported dialogue IR node: ${ir?.kind}`);
  }
}

function compileParameterParts(parameter) {
  if (parameter.parameter === "object" && parameter.index === "1") {
    return [{ kind: "playerName" }];
  }

  return textParts(renderParameterText(parameter));
}

function textParts(value) {
  return value ? [{ kind: "text", value }] : [];
}

function isRuntimeGenderCondition(condition) {
  return /^(?:PlayerParameter\(4\)|IfPcGender)$/i.test(String(condition ?? "").trim());
}

function trimTemplateParts(parts) {
  const output = [...parts];
  const first = output.find((part) => part.kind === "text");
  if (first) first.value = first.value.trimStart();
  const last = [...output].reverse().find((part) => part.kind === "text");
  if (last) last.value = last.value.trimEnd();
  return output;
}

function simplifyTemplateParts(parts) {
  const output = [];
  for (const part of parts) {
    if (part.kind === "text") {
      if (!part.value) continue;
      const previous = output[output.length - 1];
      if (previous?.kind === "text") {
        previous.value += part.value;
      } else {
        output.push({ kind: "text", value: part.value });
      }
      continue;
    }

    if (part.kind === "gender") {
      const female = simplifyTemplateParts(part.female);
      const male = simplifyTemplateParts(part.male);
      if (female.length === 0 && male.length === 0) continue;
      if (templatePartsEqual(female, male)) output.push(...female);
      else output.push({ kind: "gender", female, male });
      continue;
    }

    output.push(part);
  }

  return output;
}

function templatePartsEqual(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

export function encodeTargetTemplate(parts) {
  if (parts.length === 0) return Buffer.alloc(0);

  const chunks = [writeU32(parts.length)];
  for (const part of parts) {
    switch (part.kind) {
      case "text": {
        const bytes = Buffer.from(part.value, "utf8");
        chunks.push(writeU8(templateOp.text), writeU32(bytes.length), bytes);
        break;
      }
      case "playerName":
        chunks.push(writeU8(templateOp.playerName));
        break;
      case "gender": {
        const female = encodeTargetTemplate(part.female);
        const male = encodeTargetTemplate(part.male);
        chunks.push(
          writeU8(templateOp.gender),
          writeU32(female.length),
          female,
          writeU32(male.length),
          male);
        break;
      }
      default:
        throw new Error(`unsupported target template part: ${part.kind}`);
    }
  }

  return Buffer.concat(chunks);
}

// template expansion diagnostics
// --------------------------------
export function recordTemplateExpansionDiagnostics(records, diagnostics) {
  for (const entry of records) {
    for (const definition of packageLanguageDefinitions) {
      const source = entry[definition.property];
      if (!source) continue;

      const controlCount = countControlFlow(source.ir);
      if (controlCount === 0) continue;

      diagnostics.templateExpansion.textsWithControlFlow++;
      const variantCount = countVariants(source.ir);
      diagnostics.templateExpansion.maxObservedVariants = Math.max(
        diagnostics.templateExpansion.maxObservedVariants,
        variantCount);

      if (variantCount <= templateExpansionDiagnosticLimit) continue;
      diagnostics.templateExpansion.overflowCount++;
      if (diagnostics.templateExpansion.samples.length < 50) {
        diagnostics.templateExpansion.samples.push({
          key: entry.key,
          language: definition.code,
          variantCount,
          errors: [`variant count exceeds ${templateExpansionDiagnosticLimit}`],
          text: source.text.slice(0, 500),
        });
      }
    }
  }
}

function countControlFlow(ir) {
  switch (ir?.kind) {
    case "sequence":
      return ir.items.reduce((total, item) => total + countControlFlow(item), 0);
    case "if":
      return 1 + countControlFlow(ir.then) + countControlFlow(ir.else);
    case "switch":
      return 1 + ir.cases.reduce((total, item) => total + countControlFlow(item), 0);
    case "text":
    case "placeholder":
    case "parameter":
    case "playerName":
      return 0;
    default:
      throw new Error(`unsupported dialogue IR node: ${ir?.kind}`);
  }
}

function countVariants(ir) {
  const limit = templateExpansionDiagnosticLimit + 1;
  return Math.min(limit, countVariantsCore(ir, limit));
}

function countVariantsCore(ir, limit) {
  switch (ir?.kind) {
    case "sequence": {
      let total = 1;
      for (const item of ir.items) {
        total *= countVariantsCore(item, limit);
        if (total >= limit) return limit;
      }
      return total;
    }
    case "if":
      return Math.min(limit, countVariantsCore(ir.then, limit) + countVariantsCore(ir.else, limit));
    case "switch":
      return Math.min(limit, ir.cases.reduce((total, item) => total + countVariantsCore(item, limit), 0) || 1);
    case "text":
    case "placeholder":
    case "parameter":
    case "playerName":
      return 1;
    default:
      throw new Error(`unsupported dialogue IR node: ${ir?.kind}`);
  }
}
