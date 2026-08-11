-- Vautr server schema — audit expansion (Wave A7): org events + secret-access
-- events. Appends columns to `audit_log` (0005) so ONE durable, queryable
-- facility holds every security-relevant event (mlp-scope.md §4/§5).
--
-- Append-only numbered migration: never edit 0001..0011. This is a fresh file
-- (0012) so existing databases that already applied 0005 are upgraded in place.
--
-- New columns (metadata only — never PII, payloads, or encrypted blobs):
--   event_type    broad category: 'org' | 'secret_access' | 'auth' | ...
--   resource_type the kind of resource acted on:
--                 'project' | 'org' | 'org_member' | 'policy' | 'mfa' | 'secret' | ...
--   resource_id   UUID of the affected resource (safe: not PII, not a payload)
--   ip_address    optional source address (server-scaling metadata)

ALTER TABLE audit_log ADD COLUMN event_type TEXT;
ALTER TABLE audit_log ADD COLUMN resource_type TEXT;
ALTER TABLE audit_log ADD COLUMN resource_id TEXT;
ALTER TABLE audit_log ADD COLUMN ip_address TEXT;

-- Query-path indexes: filter by event category, resource, and actor.
CREATE INDEX IF NOT EXISTS idx_audit_log_event_type ON audit_log(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_log_resource    ON audit_log(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_actor       ON audit_log(actor);
