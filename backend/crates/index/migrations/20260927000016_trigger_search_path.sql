-- The jobs wakeup trigger names job_wakeups without a schema, so any session whose
-- search_path lacks `strata` (maintenance SQL as postgres) failed on every jobs update that
-- queued a row. Pin the function's search_path to the migration's (strata, public).
ALTER FUNCTION strata_jobs_wakeup() SET search_path FROM CURRENT;
