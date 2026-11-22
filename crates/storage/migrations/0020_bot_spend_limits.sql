-- Per-bot spending limits (ADR-0034). `limits_json` is validated by agent-core;
-- an absent row means "the safe default for new bots", `{}` means "no limits".
CREATE TABLE bot_spend_limits (
    bot_id        TEXT PRIMARY KEY REFERENCES bots(id),
    user_id       TEXT NOT NULL REFERENCES users(id),
    limits_json   TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

-- Bots that already exist keep behaving exactly as before: explicit "no limits".
INSERT INTO bot_spend_limits (bot_id, user_id, limits_json, updated_at_ms)
SELECT id, user_id, '{}', created_at_ms FROM bots;
