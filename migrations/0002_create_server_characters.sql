CREATE TABLE server_characters (
    guild_id     TEXT    NOT NULL, -- Discord guild (server) snowflake
    character_id INTEGER NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    added_by     TEXT    NOT NULL, -- Discord user snowflake
    added_at     INTEGER NOT NULL, -- unix seconds
    PRIMARY KEY (guild_id, character_id)
);

CREATE INDEX idx_server_characters_guild ON server_characters (guild_id);
