-- The user's note on a custody event ("Record a move"), written last on the custody line.
-- Existing rows get it on the next reindex of their document.
ALTER TABLE custody_events ADD COLUMN note TEXT;
