# ADR-0034: per-bot spending limits

Status: accepted. User guide: [bot spending limits](../guide/bot-spending-limits.md).

## Decisions

- **Defaults.** A new bot gets a daily hard cap of 5 USD and 10M tokens.
  Existing bots are migrated with `{}` (still unlimited, migration `0020`);
  a bot with no stored row gets the new-bot default. Limits per period (day, week, month): soft (notify at 80% and
  100%) and hard (stop), in USD and in tokens.
- **Enforcement point.** In `agent-core`, before every model call
  (`CallGuard::start`), for every run of the bot (chat, schedules, channels,
  subagents, background memory work). Usage comes from the LLM call ledger, so
  it survives restarts; an in-process reservation covers parallel runs.
  Unreadable limits fail closed. A blocked call ends the run with a
  `bot_limit_reached` card (raise / wait / stop).
- **USD caps and unknown prices.** A USD limit cannot count a model without a
  known price; the token cap does. This is documented in the guide.
- **Owner only.** Changing limits is local-owner only: refused for token-only
  callers and MCP (`OWNER_ROUTES`, `require_owner` in the handler) and for
  every paired peer role (`host_local_denial`), so nothing that a bot or a
  remote device controls can lift its own cap. Every change is audited as
  `bot.spend_limits_changed` with the old and new values. Reading limits stays
  allowed.
