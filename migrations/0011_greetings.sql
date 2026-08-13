-- Welcome and farewell announcements. Separate channels so either can be run
-- without the other; a NULL channel means that half is off.
--
-- Both need the privileged GUILD_MEMBERS intent, which the bot only requests
-- when MEMBER_INTENT is set, so these stay inert until that is turned on.

ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS welcome_channel_id BIGINT;
ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS welcome_message TEXT;
ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS welcome_card BOOLEAN;
ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS farewell_channel_id BIGINT;
ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS farewell_message TEXT;

ALTER TABLE guild_config DROP CONSTRAINT IF EXISTS welcome_message_length;
ALTER TABLE guild_config ADD CONSTRAINT welcome_message_length
    CHECK (char_length(welcome_message) BETWEEN 1 AND 1000);

ALTER TABLE guild_config DROP CONSTRAINT IF EXISTS farewell_message_length;
ALTER TABLE guild_config ADD CONSTRAINT farewell_message_length
    CHECK (char_length(farewell_message) BETWEEN 1 AND 1000);
