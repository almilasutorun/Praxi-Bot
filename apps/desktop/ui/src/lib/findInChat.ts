/**
 * Find-in-chat helpers. The DOM is never rewritten: matches are plain `Range`s painted with the
 * CSS Custom Highlight API, so Markdown rendering, selection and copy keep working. The matching
 * itself works on plain strings (one per text node) so it can be tested without a DOM.
 */
import { fold } from "./fuzzy.ts";

export interface TextMatch {
  /** Index into the list of texts. */
  node: number;
  start: number;
  end: number;
}

/** Case- and diacritic-insensitive, non-overlapping matches of `query` in each text. An empty query matches nothing. */
export function findMatches(texts: readonly string[], query: string): TextMatch[] {
  const needle = fold(query.trim());
  if (!needle) {
    return [];
  }
  const out: TextMatch[] = [];
  texts.forEach((text, node) => {
    // `fold` keeps the length, so every index points into the original text (no node is skipped
    // for containing "İ"); it also ignores diacritics and the Turkish dotless i, like the palette.
    const hay = fold(text);
    let from = 0;
    for (;;) {
      const at = hay.indexOf(needle, from);
      if (at < 0) {
        break;
      }
      out.push({ node, start: at, end: at + needle.length });
      from = at + needle.length;
    }
  });
  return out;
}

/** Next/previous match index with wrap-around; -1 when nothing matches. */
export function stepMatch(current: number, total: number, dir: 1 | -1): number {
  if (total <= 0) {
    return -1;
  }
  if (current < 0 || current >= total) {
    return dir === 1 ? 0 : total - 1;
  }
  return (current + dir + total) % total;
}

/** Text nodes under `root` that are visible chat text (no scripts, styles or hidden controls). */
export function collectTextNodes(root: HTMLElement): Text[] {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (!parent || parent.closest("script,style,[aria-hidden='true'],[data-find-skip]")) {
        return NodeFilter.FILTER_REJECT;
      }
      return node.nodeValue ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
    }
  });
  const nodes: Text[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    nodes.push(n as Text);
  }
  return nodes;
}

export function matchRanges(nodes: readonly Text[], matches: readonly TextMatch[]): Range[] {
  return matches.map((m) => {
    const r = document.createRange();
    r.setStart(nodes[m.node], m.start);
    r.setEnd(nodes[m.node], m.end);
    return r;
  });
}

export const FIND_ALL = "praxi-find";
export const FIND_CURRENT = "praxi-find-current";

type HighlightRegistry = { set(name: string, h: unknown): void; delete(name: string): void };

function registry(): { reg: HighlightRegistry; Highlight: new (...r: Range[]) => unknown } | null {
  const reg = (globalThis.CSS as unknown as { highlights?: HighlightRegistry } | undefined)?.highlights;
  const Highlight = (globalThis as unknown as { Highlight?: new (...r: Range[]) => unknown }).Highlight;
  return reg && Highlight ? { reg, Highlight } : null;
}

/** Paint all matches and the current one; returns false when the engine lacks the Highlight API. */
export function paintHighlights(ranges: readonly Range[], current: number): boolean {
  const api = registry();
  if (!api) {
    return false;
  }
  if (ranges.length === 0) {
    clearHighlights();
    return true;
  }
  api.reg.set(FIND_ALL, new api.Highlight(...ranges));
  if (current >= 0 && ranges[current]) {
    api.reg.set(FIND_CURRENT, new api.Highlight(ranges[current]));
  } else {
    api.reg.delete(FIND_CURRENT);
  }
  return true;
}

export function clearHighlights(): void {
  const api = registry();
  api?.reg.delete(FIND_ALL);
  api?.reg.delete(FIND_CURRENT);
}
