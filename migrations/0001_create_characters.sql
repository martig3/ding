CREATE TABLE characters (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    first_name TEXT    NOT NULL,
    last_name  TEXT    NOT NULL,
    name       TEXT    NOT NULL,
    level      INTEGER NOT NULL,
    item_level INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, -- unix seconds
    UNIQUE (first_name, last_name)
);
