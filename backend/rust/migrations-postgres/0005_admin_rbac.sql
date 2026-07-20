-- Granular admin RBAC for staff principals: a JSONB array of fixed-registry
-- `{family}:read` / `{family}:write` permission strings. `is_admin` bypasses
-- the registry entirely; staff carry only what an operator granted. The
-- registry itself is code-owned (crates/domain/src/admin/permissions.rs) and
-- write-validated there, so the schema only pins the shape.
ALTER TABLE users
    ADD COLUMN admin_permissions JSONB NOT NULL DEFAULT '[]'::jsonb
        CONSTRAINT chk_user_admin_permissions
            CHECK (jsonb_typeof(admin_permissions) = 'array');
