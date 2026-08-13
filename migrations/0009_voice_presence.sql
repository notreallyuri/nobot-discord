-- 24/7 mode. `stay_connected` and `idle_timeout_secs` are settings; the two
-- channel ids are remembered state, written on join so the bot can rejoin
-- after a restart. They are only meaningful while stay_connected is on.

ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS stay_connected BOOLEAN;
ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS idle_timeout_secs INTEGER;
ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS voice_channel_id BIGINT;
ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS voice_text_channel_id BIGINT;

ALTER TABLE guild_config DROP CONSTRAINT IF EXISTS idle_timeout_range;
ALTER TABLE guild_config ADD CONSTRAINT idle_timeout_range
    CHECK (idle_timeout_secs BETWEEN 10 AND 86400);
