import { fold } from "./fuzzy.ts";
import { mentionHandle } from "./slash.ts";

export interface MentionPart {
  text: string;
  /** Colour of the mentioned bot (set for `@Name` parts only). */
  color?: string;
}

/** Split a message into plain text and `@Name` parts of known bots (case-insensitive, longest handle first). */
export function splitMentions(text: string, bots: ReadonlyArray<{ name: string; color?: string }>): MentionPart[] {
  const known = bots
    .map((b) => ({ handle: fold(mentionHandle(b.name)), color: b.color }))
    .filter((b) => b.handle !== "" && b.color)
    .sort((a, b) => b.handle.length - a.handle.length);
  if (known.length === 0 || !text.includes("@")) return [{ text }];
  const parts: MentionPart[] = [];
  let plain = "";
  let i = 0;
  while (i < text.length) {
    const prev = i === 0 ? " " : (text[i - 1] ?? " ");
    if (text[i] === "@" && /[\s([{]/.test(prev)) {
      const rest = fold(text.slice(i + 1));
      const hit = known.find(
        (b) => rest.startsWith(b.handle) && !/[\p{L}\p{N}_-]/u.test(rest.charAt(b.handle.length))
      );
      if (hit) {
        if (plain) parts.push({ text: plain });
        plain = "";
        parts.push({ text: text.slice(i, i + 1 + hit.handle.length), color: hit.color });
        i += 1 + hit.handle.length;
        continue;
      }
    }
    plain += text[i];
    i += 1;
  }
  if (plain) parts.push({ text: plain });
  return parts;
}
