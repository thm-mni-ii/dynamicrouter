-- Add token tracking columns to request table
ALTER TABLE request ADD COLUMN prompt_tokens INT NOT NULL DEFAULT 0;
ALTER TABLE request ADD COLUMN completion_tokens INT NOT NULL DEFAULT 0;
ALTER TABLE request ADD COLUMN total_tokens INT NOT NULL DEFAULT 0;

-- Create rate_limits table
CREATE TABLE IF NOT EXISTS rate_limits (
    id TEXT PRIMARY KEY NOT NULL,
    user TEXT,                          -- Specific username or NULL/'*' for all users
    model TEXT,                         -- Specific model ID or NULL/'*' for all models
    time_window TEXT NOT NULL,          -- 'minute', 'hour', 'day', 'week', 'month', 'permanent'
    limit_value INT NOT NULL,           -- Limit threshold
    unit TEXT NOT NULL DEFAULT 'tokens',-- 'tokens', 'prompt_tokens', 'completion_tokens', 'requests'
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_rate_limits_user ON rate_limits (user);
CREATE INDEX IF NOT EXISTS idx_rate_limits_model ON rate_limits (model);
CREATE INDEX IF NOT EXISTS idx_rate_limits_window ON rate_limits (time_window);

-- High performance covering indexes for rolling window aggregation
CREATE INDEX IF NOT EXISTS idx_request_user_enqueue_tokens 
    ON request (user, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);

CREATE INDEX IF NOT EXISTS idx_request_user_model_enqueue_tokens 
    ON request (user, model, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);
