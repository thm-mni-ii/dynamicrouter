-- Persistent sessions table to preserve login sessions across server restarts
CREATE TABLE IF NOT EXISTS sessions (
    session_token TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    email TEXT,
    name TEXT,
    roles TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_username ON sessions (username);
