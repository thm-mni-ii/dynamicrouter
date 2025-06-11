ALTER TABLE instance RENAME TO instance_old;
CREATE TABLE instance (
    id TEXT PRIMARY KEY NOT NULL,
    model TEXT NOT NULL,
    data BLOB NOT NULL
);
INSERT INTO instance SELECT id, model, data FROM instance_old;
DROP TABLE instance_old;

CREATE TABLE request (
    id TEXT PRIMARY KEY NOT NULL,
    model TEXT NOT NULL,
    enqueue INT NOT NULL,
    done INT NULL,
    instance TEXT NOT NULL REFERENCES instance(id) ON UPDATE CASCADE ON DELETE CASCADE
);
