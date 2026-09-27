-- Crash-safe sync push results (docs/ARCHITECTURE.md "Vault store" and "Sync and events").
--
-- `vault_head` is the vault commit the index last committed with. Every vault write stores it
-- in the same transaction as its index update, its change-log rows and — for a pushed op — the
-- op's `idempotency` row. After a crash between the git commit and the database commit, git's
-- HEAD is ahead of `vault_head`: reconciliation walks the commits after it, re-inserts the op
-- results recorded in their `Strata-Op` trailers, re-derives the index and moves `vault_head`
-- to HEAD, so a replayed op is answered with its original result instead of being applied
-- again. NULL until the first write after this migration (reconciliation then scans the
-- history once).

ALTER TABLE sync_epochs ADD COLUMN vault_head text CHECK (vault_head ~ '^[0-9a-f]{40}$');
