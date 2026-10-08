-- Add status enum column with CHECK constraint to request table
ALTER TABLE request ADD COLUMN status TEXT NOT NULL DEFAULT 'completed'
    CHECK(status IN ('queued', 'completed', 'failed', 'aborted', 'unknown'));

-- Update historical requests with indeterminate state (done IS NULL) to 'unknown'
UPDATE request SET status = 'unknown', done = enqueue WHERE done IS NULL;

-- Indexes for status-based queue and filter lookups
CREATE INDEX IF NOT EXISTS idx_request_status_enqueue ON request (status, enqueue DESC);
CREATE INDEX IF NOT EXISTS idx_request_model_status_enqueue ON request (model, status, enqueue DESC);
CREATE INDEX IF NOT EXISTS idx_request_user_status_enqueue ON request (user, status, enqueue DESC);
