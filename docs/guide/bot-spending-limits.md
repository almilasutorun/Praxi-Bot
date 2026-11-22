# Bot spending limits

Every bot can have a daily, weekly and monthly limit on what its model calls
may use. Open the bot, choose **Edit**, then **Limits**.

## What you can set

For each period (per day, per week, per month; local calendar, the week starts
on Monday) there are four optional fields:

| Field | What it does |
| --- | --- |
| Warn at (USD) / (tokens) | Notifies you at 80% and at 100% of this amount. The bot keeps working. |
| Stop at (USD) / (tokens) | The run stops **before** a model call that would go past this amount. |

Empty means no limit. A warning level cannot be above its stop level.

## The default for new bots

A bot that was never configured is capped at **5 USD or 10 million tokens per
day**. The Limits section says so and you can raise or remove it. Bots that
existed before this feature keep running without limits until you set some.

## What happens at the limit

The run ends with a card, "Daily limit reached" (or weekly, monthly), and three
choices: **Raise limit** (opens the Limits section), **Wait** (the counter
resets at the time shown) or **Stop** (leave it stopped). Nothing is retried in
a loop and the model is not called. Scheduled runs, channel messages (for
example Telegram), subagents and background memory work follow the same limits.

## How usage is counted

- Money comes from the recorded cost of each call in the
  [LLM call ledger](llm-call-ledger.md). Models without a known price add tokens
  only. A money (USD) limit therefore cannot count them: it does not stop a
  model whose price is unknown. That is why every new bot also gets a token
  cap, which counts every model. Set a token limit as well when you use
  models without a known price.
- A call whose provider reported no usage is counted with a cautious estimate,
  so missing data never means free. The page shows how many calls were counted
  this way.
- Counters live in the ledger, so they survive restarts. Parallel runs of one
  bot share the same remaining amount.

## Who can change limits

Only the owner (the desktop app on this computer). A paired phone or another
desktop cannot change or erase a limit either; they can only read it. A bot, a CLI engine, the MCP
mode or anything holding only the API token cannot change or lift a limit, and
every change is written to the audit log (`bot.spend_limits_changed`, with the
old and the new values). Limits of a remote host are changed on that host.

The installation-wide monthly limit on the Usage page still applies on top.

API: `GET` and `PUT /v1/bots/{id}/spend-limits`.
