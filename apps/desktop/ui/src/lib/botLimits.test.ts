import assert from "node:assert/strict";
import { test } from "node:test";
import { draftFromLimits, limitsFromDraft, parseLimitReached } from "./botLimits.ts";

const empty = { soft_usd: null, hard_usd: null, soft_tokens: null, hard_tokens: null };

test("the daemon's typed stop is recognised inside a wrapped run error", () => {
  const m =
    "step 'model' failed: invalid request: [bot_limit_reached period=weekly metric=tokens resets_ms=1795381200000] This bot's weekly limit";
  assert.deepEqual(parseLimitReached(m), { period: "weekly", metric: "tokens", resetsMs: 1795381200000 });
  assert.equal(parseLimitReached("spend_limit_reached"), null);
  assert.equal(parseLimitReached("[bot_limit_reached period=unknown metric=unknown resets_ms=0] x")?.resetsMs, null);
});

test("limit form: empty is unlimited, junk and soft-above-hard name the bad field", () => {
  const draft = draftFromLimits({ daily: empty, weekly: empty, monthly: empty });
  const ok = limitsFromDraft({ ...draft, daily: { ...draft.daily, hard_usd: "7,5", hard_tokens: "2000000" } });
  assert.ok(ok.ok && ok.limits.daily?.hard_usd === 7.5 && ok.limits.daily?.hard_tokens === 2_000_000);
  assert.deepEqual(limitsFromDraft({ ...draft, weekly: { ...draft.weekly, hard_usd: "0" } }), { ok: false, period: "weekly", field: "hard_usd" });
  assert.deepEqual(limitsFromDraft({ ...draft, monthly: { ...draft.monthly, hard_tokens: "1.5" } }), { ok: false, period: "monthly", field: "hard_tokens" });
  assert.deepEqual(limitsFromDraft({ ...draft, daily: { ...draft.daily, soft_usd: "9", hard_usd: "3" } }), { ok: false, period: "daily", field: "order" });
});
