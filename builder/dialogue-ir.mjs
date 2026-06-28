import { createBinaryReader, stringById } from "./binary.mjs";

export function decodeDialogueIr(buffer, start, end, strings, label) {
  const reader = createBinaryReader(buffer, start, end, label);
  const node = decodeIrNode(reader, strings, 0);
  if (reader.offset !== end) {
    throw new Error(`${label}: dialogue IR decoder left ${end - reader.offset} trailing bytes`);
  }

  return node;
}

function decodeIrNode(reader, strings, depth) {
  if (depth > 256) throw new Error(`${reader.label}: dialogue IR nesting exceeds 256`);
  const tag = reader.readU8();
  switch (tag) {
    case 1: {
      const count = reader.readU32();
      if (count > 100000) throw new Error(`${reader.label}: unreasonable IR sequence item count ${count}`);
      const items = [];
      for (let index = 0; index < count; index++) {
        items.push(decodeIrNode(reader, strings, depth + 1));
      }

      return { kind: "sequence", items };
    }
    case 2:
      return { kind: "text", value: stringById(strings, reader.readU32(), `${reader.label}:text`) };
    case 3:
      return { kind: "placeholder", value: stringById(strings, reader.readU32(), `${reader.label}:placeholder`) };
    case 4:
      return {
        kind: "parameter",
        parameter: stringById(strings, reader.readU32(), `${reader.label}:parameterKind`),
        index: stringById(strings, reader.readU32(), `${reader.label}:parameterIndex`),
      };
    case 5:
      return { kind: "playerName" };
    case 6:
      return {
        kind: "if",
        condition: stringById(strings, reader.readU32(), `${reader.label}:ifCondition`),
        then: decodeIrNode(reader, strings, depth + 1),
        else: decodeIrNode(reader, strings, depth + 1),
      };
    case 7: {
      const condition = stringById(strings, reader.readU32(), `${reader.label}:switchCondition`);
      const count = reader.readU32();
      if (count > 100000) throw new Error(`${reader.label}: unreasonable IR switch case count ${count}`);
      const cases = [];
      for (let index = 0; index < count; index++) {
        cases.push(decodeIrNode(reader, strings, depth + 1));
      }

      return { kind: "switch", condition, cases };
    }
    default:
      throw new Error(`${reader.label}: unsupported dialogue IR tag ${tag} at byte ${reader.offset - 1}`);
  }
}
