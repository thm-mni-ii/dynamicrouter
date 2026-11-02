-- Create groups table
CREATE TABLE IF NOT EXISTS groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    limit_to_models TEXT,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_groups_name ON groups (name);

-- Create group_members table for manual group assignments
CREATE TABLE IF NOT EXISTS group_members (
    group_id TEXT NOT NULL REFERENCES groups(id) ON UPDATE CASCADE ON DELETE CASCADE,
    username TEXT NOT NULL REFERENCES users(username) ON UPDATE CASCADE ON DELETE CASCADE,
    created_at INT NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (group_id, username)
);

CREATE INDEX IF NOT EXISTS idx_group_members_username ON group_members (username);
CREATE INDEX IF NOT EXISTS idx_group_members_group_id ON group_members (group_id);

-- Create group_oidc_rules table for CEL-based OIDC profile matching
CREATE TABLE IF NOT EXISTS group_oidc_rules (
    id TEXT PRIMARY KEY NOT NULL,
    group_id TEXT NOT NULL REFERENCES groups(id) ON UPDATE CASCADE ON DELETE CASCADE,
    name TEXT,
    cel_expression TEXT NOT NULL,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_group_oidc_rules_group_id ON group_oidc_rules (group_id);

-- Add group support to rate_limits
ALTER TABLE rate_limits ADD COLUMN group_name TEXT REFERENCES groups(name) ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE rate_limits ADD COLUMN scope_mode TEXT NOT NULL DEFAULT 'per_user';
CREATE INDEX IF NOT EXISTS idx_rate_limits_group_name ON rate_limits (group_name);

-- Add groups column to sessions table for session-only OIDC group caching
ALTER TABLE sessions ADD COLUMN groups TEXT;
