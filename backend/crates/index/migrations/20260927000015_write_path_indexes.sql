-- Indexes for the per-write index update (docs/ARCHITECTURE.md "Vault store", write cost):
-- every lookup a note write makes is an index probe, so a write costs the same whatever the
-- size of the vault.
--
-- `dedupe_keys_item`: deleting a note's keys before re-deriving them (by item ID; the
--                     primary key leads with `kind`).
-- `dedupe_keep_both_a` / `_b`: the keep-both pairs involving one item (the duplicate check
--                     on create), matched on either side.
CREATE INDEX dedupe_keys_item ON dedupe_keys (user_id, item_id);
CREATE INDEX dedupe_keep_both_a ON dedupe_keep_both (user_id, a_id);
CREATE INDEX dedupe_keep_both_b ON dedupe_keep_both (user_id, b_id);
