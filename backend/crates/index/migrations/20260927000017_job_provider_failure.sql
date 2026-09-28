-- Jobs that ended because the AI provider was down (unavailable, timed out, login broken) are
-- marked so they can be requeued automatically once the provider works again (owner decision
-- 2026-09-28, DECISIONS.md); any failed job can also be retried from the app.
ALTER TABLE jobs ADD COLUMN provider_failure boolean NOT NULL DEFAULT false;
CREATE INDEX jobs_failed ON jobs (user_id, updated) WHERE status = 'failed';
