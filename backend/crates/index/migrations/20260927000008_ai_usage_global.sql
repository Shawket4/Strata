-- AI budget guard (PLAN §9.1): the global daily cap needs today's totals across all users, but
-- `ai_usage` is user-owned (RLS), so no scope can sum it. This global table holds only per-day
-- counters — no user IDs, no content — and is updated in the same transaction as the caller's
-- `ai_usage` row (strata_app, inside any user's scope).

CREATE TABLE ai_usage_global (
    day             date PRIMARY KEY,
    calls           bigint NOT NULL DEFAULT 0 CHECK (calls >= 0),
    input_tokens    bigint NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    output_tokens   bigint NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    -- Estimated cost in micro-USD (integer money, no floats).
    est_cost_micros bigint NOT NULL DEFAULT 0 CHECK (est_cost_micros >= 0)
);

REVOKE ALL ON ai_usage_global FROM PUBLIC;
GRANT SELECT, INSERT, UPDATE ON ai_usage_global TO strata_app;
