import { PERMISSION_CATEGORIES } from "../api/types.ts";
import {
  type BotDraft,
  formatModelRef,
  type ModelRef,
  parseModelRef,
  type PresetChoice
} from "./botProfile.ts";

/**
 * State rules of the "Edit bot" dialog (owner feedback 244, point 5): its sections, what counts
 * as unsaved, what blocks saving, and what closing does.  The dialog component only renders.
 */

export const BOT_EDITOR_SECTIONS = [
  "identity",
  "models",
  "permissions",
  "skills",
  "apps",
  "limits",
  "advanced"
] as const;

export type BotEditorSection = (typeof BOT_EDITOR_SECTIONS)[number];

/** A section name from outside (a review URL): only known sections pass. */
export function editorSection(value: string | null | undefined): BotEditorSection | undefined {
  return BOT_EDITOR_SECTIONS.find((s) => s === value);
}

/** Arrow / Home / End navigation of the section list (wraps around). */
export function stepSection(
  current: BotEditorSection,
  key: string
): BotEditorSection | null {
  const i = BOT_EDITOR_SECTIONS.indexOf(current);
  const n = BOT_EDITOR_SECTIONS.length;
  const at = (j: number): BotEditorSection => BOT_EDITOR_SECTIONS[(j + n) % n] ?? current;
  switch (key) {
    case "ArrowDown":
    case "ArrowRight":
      return at(i + 1);
    case "ArrowUp":
    case "ArrowLeft":
      return at(i - 1);
    case "Home":
      return at(0);
    case "End":
      return at(n - 1);
    default:
      return null;
  }
}

function sameRef(a: ModelRef, b: ModelRef): boolean {
  return formatModelRef(a) === formatModelRef(b);
}

export interface EditorSnapshot {
  draft: BotDraft;
  initialDraft: BotDraft;
  apps: Record<string, boolean>;
  initialApps: Record<string, boolean>;
  skillsDirty: boolean;
  /** Hand-typed model ids that are not valid yet (kept as typed, see `invalidCustomRoles`). */
  customText?: CustomText;
  /** Text typed in the "Other model id" fallback field that was not added yet. */
  pendingFallback?: string;
}

function appsChanged(a: Record<string, boolean>, b: Record<string, boolean>): boolean {
  const names = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const name of names) {
    if ((a[name] ?? false) !== (b[name] ?? false)) return true;
  }
  return false;
}

/** Which sections hold unsaved changes. */
export function dirtySections(s: EditorSnapshot): BotEditorSection[] {
  const { draft: d, initialDraft: i } = s;
  const out: BotEditorSection[] = [];
  if (
    d.name.trim() !== i.name.trim() ||
    d.persona !== i.persona ||
    d.shape !== i.shape ||
    d.color.toLowerCase() !== i.color.toLowerCase() ||
    (d.avatar ?? null) !== (i.avatar ?? null)
  ) {
    out.push("identity");
  }
  if (
    !sameRef(d.executor, i.executor) ||
    !sameRef(d.planner, i.planner) ||
    !sameRef(d.classifier, i.classifier) ||
    fallbackList(d.fallback).join(",") !== fallbackList(i.fallback).join(",") ||
    (s.pendingFallback ?? "").trim() !== ""
  ) {
    out.push("models");
  }
  if (d.preset !== i.preset || PERMISSION_CATEGORIES.some((c) => d.categories[c] !== i.categories[c])) {
    out.push("permissions");
  }
  if (s.skillsDirty) out.push("skills");
  if (appsChanged(s.apps, s.initialApps)) out.push("apps");
  if (invalidCustomRoles(s.customText ?? {}).length > 0) out.push("advanced");
  return out;
}

export type EditorProblem =
  | "nameMissing"
  | "executorMissing"
  | "customInvalid"
  | "fallbackPending";

/** What blocks Save, and the section that fixes it. */
export function editorProblems(
  draft: BotDraft,
  customText: CustomText = {},
  pendingFallback = ""
): Array<{ problem: EditorProblem; section: BotEditorSection }> {
  const out: Array<{ problem: EditorProblem; section: BotEditorSection }> = [];
  if (draft.name.trim() === "") out.push({ problem: "nameMissing", section: "identity" });
  if (draft.executor.model.trim() === "") {
    out.push({ problem: "executorMissing", section: "models" });
  }
  if (invalidCustomRoles(customText).length > 0) {
    out.push({ problem: "customInvalid", section: "advanced" });
  }
  // Typed but not added (valid or not): Save would silently leave it out.
  if (pendingFallback.trim() !== "") {
    out.push({ problem: "fallbackPending", section: "models" });
  }
  return out;
}

/**
 * Closing the dialog (Esc, backdrop, the X, Cancel): never while a save is in flight, and only
 * after a confirmation when something is unsaved.
 */
export function closeAction(dirty: boolean, saving: boolean): "close" | "confirm" | "wait" {
  if (saving) return "wait";
  return dirty ? "confirm" : "close";
}

// --- deleting a bot --------------------------------------------------------

export type DeleteBlock = "chats" | "runs" | null;

/**
 * Why Delete is unavailable: the daemon refuses a bot that still has chats or ANY retained run
 * (finished and scheduled runs included).  `runs` is `null` while the run list has not loaded:
 * Delete stays available then and the daemon's own `409` is shown.
 */
export function deleteBlock(chatCount: number, runCount: number | null): DeleteBlock {
  if (chatCount > 0) return "chats";
  if (runCount !== null && runCount > 0) return "runs";
  return null;
}

// --- fallback models -------------------------------------------------------

/** The stored comma list as ordered, de-duplicated refs. */
export function fallbackList(text: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const part of text.split(",")) {
    const ref = part.trim();
    if (ref && !seen.has(ref)) {
      seen.add(ref);
      out.push(ref);
    }
  }
  return out;
}

export function addFallback(text: string, ref: string): string {
  return fallbackList(`${text},${ref}`).join(", ");
}

export function removeFallback(text: string, ref: string): string {
  return fallbackList(text)
    .filter((r) => r !== ref)
    .join(", ");
}

// --- custom model id (the "Advanced" escape hatch) -------------------------

export type CustomRefIssue = "empty" | "spaces" | "noModel";

/**
 * Check a hand-typed `provider/model` id.  The model part may itself contain slashes
 * (`openrouter/meta-llama/llama-3.3`); a missing provider is allowed (default provider).
 */
export function customRefIssue(text: string): CustomRefIssue | null {
  const t = text.trim();
  if (!t) return "empty";
  if (/\s/.test(t)) return "spaces";
  const ref = parseModelRef(t);
  if (!ref.model.trim()) return "noModel";
  return null;
}

/** A typed id as the `ModelRef` a draft stores; `null` while it is not usable. */
export function customRef(text: string): ModelRef | null {
  return customRefIssue(text) ? null : parseModelRef(text.trim());
}

// --- permission mode chip --------------------------------------------------

export type PermissionTone = "locked" | "guarded" | "open" | "custom";

/** How the permission chip is tinted: what the bot may do without asking. */
export function permissionTone(preset: PresetChoice): PermissionTone {
  switch (preset) {
    case "read-only":
      return "locked";
    case "full":
      return "open";
    case "custom":
      return "custom";
    case "approve-destructive":
    case "read-and-draft":
      return "guarded";
  }
}

/** Roles with a hand-typed model id (Advanced). */
export type CustomRole = "executor" | "planner" | "classifier";

/** The text of a custom id field while it differs from the draft's model (kept as typed). */
export type CustomText = Partial<Record<CustomRole, string>>;

/**
 * Typed ids that are not usable: they block Save and count as unsaved data.  An empty field is
 * not "invalid" here (an optional role may be cleared; an empty main model is `executorMissing`).
 */
export function invalidCustomRoles(text: CustomText): CustomRole[] {
  return (Object.keys(text) as CustomRole[]).filter((role) => {
    const raw = text[role];
    return raw !== undefined && raw.trim() !== "" && customRef(raw) === null;
  });
}

/**
 * One keystroke in a custom id field: the draft value to store (`null` = keep the draft as it is
 * while the text is invalid) and the text to keep (`undefined` = the draft's own text is shown).
 */
export function applyCustomText(
  text: string
): { ref: ModelRef | null; keep: string | undefined } {
  const ref = text.trim() === "" ? { provider: "", model: "" } : customRef(text);
  const canonical = ref ? formatModelRef(ref) : undefined;
  return { ref, keep: ref && text === canonical ? undefined : text };
}

// --- fallback entry ----------------------------------------------------------

export type FallbackIssue = CustomRefIssue;

/**
 * Check a typed fallback list (`provider/model`, comma separated, any id - not only the catalog's).
 * Returns the refs to add, or the first issue.
 */
export function parseFallbackInput(
  text: string
): { refs: string[] } | { issue: CustomRefIssue } {
  const parts = text.split(",").map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return { issue: "empty" };
  for (const part of parts) {
    const issue = customRefIssue(part);
    if (issue) return { issue };
  }
  return { refs: parts };
}
