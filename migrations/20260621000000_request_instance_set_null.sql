-- Decouple request audit/ledger from ephemeral instance deletion (ON DELETE SET NULL)
PRAGMA defer_foreign_keys = ON;

CREATE TABLE IF NOT EXISTS request_new (
    id TEXT PRIMARY KEY NOT NULL,
    model TEXT NOT NULL,
    enqueue INT NOT NULL,
    done INT,
    instance TEXT REFERENCES instance(id) ON UPDATE CASCADE ON DELETE SET NULL,
    user TEXT,
    request_body TEXT,
    response_body TEXT,
    prompt_tokens INT NOT NULL DEFAULT 0,
    completion_tokens INT NOT NULL DEFAULT 0,
    total_tokens INT NOT NULL DEFAULT 0,
    token_hash TEXT,
    status TEXT NOT NULL DEFAULT 'completed'
    CHECK(status IN ('queued', 'completed', 'failed', 'aborted', 'unknown'))
);

INSERT INTO request_new (id, model, enqueue, done, instance, user, request_body, response_body, prompt_tokens, completion_tokens, total_tokens, token_hash, status)
SELECT 
    id, 
    model, 
    enqueue, 
    done, 
    CASE WHEN instance IN (SELECT id FROM instance) THEN instance ELSE NULL END,
    user, 
    request_body, 
    response_body, 
    prompt_tokens, 
    completion_tokens, 
    total_tokens, 
    token_hash, 
    status
FROM request;

DROP TABLE request;

ALTER TABLE request_new RENAME TO request;

CREATE INDEX IF NOT EXISTS idx_request_model_enqueue ON request (model, enqueue DESC);
CREATE INDEX IF NOT EXISTS idx_request_user_enqueue_tokens 
    ON request (user, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);
CREATE INDEX IF NOT EXISTS idx_request_user_model_enqueue_tokens 
    ON request (user, model, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);
CREATE INDEX IF NOT EXISTS idx_request_token_enqueue 
    ON request (token_hash, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);
CREATE INDEX IF NOT EXISTS idx_request_token_model_enqueue 
    ON request (token_hash, model, enqueue DESC, total_tokens, prompt_tokens, completion_tokens);
CREATE INDEX IF NOT EXISTS idx_request_status_enqueue ON request (status, enqueue DESC);
CREATE INDEX IF NOT EXISTS idx_request_model_status_enqueue ON request (model, status, enqueue DESC);
CREATE INDEX IF NOT EXISTS idx_request_user_status_enqueue ON request (user, status, enqueue DESC);

PRAGMA defer_foreign_keys = OFF;
