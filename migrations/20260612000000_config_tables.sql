CREATE TABLE IF NOT EXISTS users (
    username TEXT PRIMARY KEY NOT NULL,
    limit_to_models TEXT,
    log_request_body BOOLEAN,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS user_tokens (
    token_hash TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL REFERENCES users(username) ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS openai_apis (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    token TEXT,
    prefix TEXT,
    models TEXT,
    timeout INT,
    headers TEXT,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS models (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT,
    timeout INT,
    limit_to_users TEXT,
    hidden BOOLEAN,
    handler_type TEXT NOT NULL DEFAULT 'spawner',
    spawner_backends TEXT,
    min_scale INT NOT NULL DEFAULT 0,
    max_scale INT NOT NULL DEFAULT 1,
    queue_length_for_scale_up INT NOT NULL DEFAULT 1,
    openai_api_id TEXT REFERENCES openai_apis(id) ON UPDATE CASCADE ON DELETE SET NULL,
    openai_url TEXT,
    openai_token TEXT,
    openai_model TEXT,
    openai_headers TEXT,
    openai_timeout INT,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS aliases (
    alias TEXT PRIMARY KEY NOT NULL,
    target TEXT NOT NULL,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS backends (
    id TEXT PRIMARY KEY NOT NULL,
    backend_type TEXT NOT NULL,
    image TEXT NOT NULL,
    use_for TEXT,
    created_at INT NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS backend_model_configs (
    backend_id TEXT NOT NULL REFERENCES backends(id) ON UPDATE CASCADE ON DELETE CASCADE,
    model TEXT NOT NULL,
    arguments TEXT,
    environment_variables TEXT,
    ctx_length INT,
    PRIMARY KEY (backend_id, model)
);

CREATE TABLE IF NOT EXISTS factory_gpus (
    factory_type TEXT NOT NULL,
    id TEXT NOT NULL,
    memory INT NOT NULL,
    price REAL,
    max_count INT,
    model TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (factory_type, id, model)
);
