-- Vautr server schema — per-user monotonic sequence for items (VTRFIX-BUG-C03).
-- Append-only. Never edit 0001..0015.
--
-- Before this migration the pull cursor was the item's per-user `version`
-- (which is an OCC counter that gets reset by tombstones and recreated by
-- edits). A client with cursor=1 could never see newly-inserted items at
-- version=1. `seq` is a strictly increasing, per-user sequence that never
-- decreases and is only assigned at INSERT time.

ALTER TABLE items ADD COLUMN seq INTEGER;

-- Backfill: assign seq per user in rowid insertion order. rowid is safe on
-- this table even though the PK is composite — SQLite only withholds it when
-- the table is declared WITHOUT ROWID (which this one is not).
UPDATE items SET seq = (
    SELECT COUNT(*) FROM items i2
    WHERE i2.user_id = items.user_id
      AND i2.rowid <= items.rowid
);

CREATE INDEX IF NOT EXISTS idx_items_user_seq ON items(user_id, seq);
