-- Enhance user_tokens table with metadata, restrictions, prefix/suffix and expiration
ALTER TABLE user_tokens ADD COLUMN name TEXT;
ALTER TABLE user_tokens ADD COLUMN token_prefix TEXT;
ALTER TABLE user_tokens ADD COLUMN token_suffix TEXT;
ALTER TABLE user_tokens ADD COLUMN limit_to_models TEXT;
ALTER TABLE user_tokens ADD COLUMN expires_at INT;
ALTER TABLE user_tokens ADD COLUMN allowed_ips TEXT;
ALTER TABLE user_tokens ADD COLUMN created_at INT;

-- Add token_hash support to rate_limits
ALTER TABLE rate_limits ADD COLUMN token_hash TEXT REFERENCES user_tokens(token_hash) ON UPDATE CASCADE ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_rate_limits_token ON rate_limits (token_hash);

-- Add token tracking column to request table with covering indexes
ALTER TABLE request ADD COLUMN token_hash TEXT;
CREATE INDEX IF NOT EXISTS idx_request_token_enqueue 
    ON request (token_hash, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);
CREATE INDEX IF NOT EXISTS idx_request_token_model_enqueue 
    ON request (token_hash, model, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);
