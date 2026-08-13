-- Roles handed out automatically when someone joins. An array rather than its
-- own table because there is no per-role metadata to store, unlike role_menu.
-- NULL or empty means the feature is off.

ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS autorole_ids BIGINT[];

ALTER TABLE guild_config DROP CONSTRAINT IF EXISTS autorole_count;
ALTER TABLE guild_config ADD CONSTRAINT autorole_count
    CHECK (autorole_ids IS NULL OR array_length(autorole_ids, 1) <= 10);
