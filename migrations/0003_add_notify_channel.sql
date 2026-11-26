-- Channel where level-up notifications for this (server, character) are posted.
-- NULL for rows tracked before this column existed: those servers get no notifications.
ALTER TABLE server_characters ADD COLUMN channel_id TEXT;
