-- Vautr server schema — share owner immutability (VTRFIX-SEC-H06).
-- Append-only. Never edit 0001..0017.

-- The application-level upsert in `repository/sharing.rs::create_share`
-- already stops overwriting `owner_user_id`, but a trigger enforces the
-- invariant at the DB layer so no future code path can bypass it.
CREATE TRIGGER IF NOT EXISTS trg_share_no_owner_change
BEFORE UPDATE ON shares
FOR EACH ROW
WHEN OLD.owner_user_id != NEW.owner_user_id
BEGIN
  SELECT RAISE(ABORT, 'share owner is immutable');
END;
