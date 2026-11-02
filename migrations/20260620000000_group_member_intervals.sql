-- Migration to support group membership intervals and distinction between manual and OIDC dynamic assignments
CREATE TABLE IF NOT EXISTS group_members_new (
    id TEXT PRIMARY KEY NOT NULL,
    group_id TEXT NOT NULL REFERENCES groups(id) ON UPDATE CASCADE ON DELETE CASCADE,
    username TEXT NOT NULL REFERENCES users(username) ON UPDATE CASCADE ON DELETE CASCADE,
    is_manual BOOLEAN NOT NULL DEFAULT 1,
    joined_at INT NOT NULL DEFAULT (unixepoch()),
    left_at INT,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

INSERT INTO group_members_new (id, group_id, username, is_manual, joined_at, left_at, created_at)
SELECT
    group_id || '_' || username || '_' || created_at,
    group_id,
    username,
    1,
    created_at,
    NULL,
    created_at
FROM group_members;

DROP TABLE group_members;

ALTER TABLE group_members_new RENAME TO group_members;

CREATE INDEX IF NOT EXISTS idx_group_members_username ON group_members (username);
CREATE INDEX IF NOT EXISTS idx_group_members_group_id ON group_members (group_id);
CREATE INDEX IF NOT EXISTS idx_group_members_active ON group_members (group_id, username, left_at);
CREATE INDEX IF NOT EXISTS idx_group_members_interval ON group_members (group_id, username, joined_at, left_at);
