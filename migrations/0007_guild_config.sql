-- Per-guild overrides. A guild only gets a row once it changes something, and
-- every setting is nullable: NULL means "use the bot's default", so the table
-- never has to be seeded and defaults can move without a data migration.
--
-- Later features add their own columns here the way `profile` grew.

CREATE TABLE IF NOT EXISTS guild_config (
    guild_id         BIGINT NOT NULL PRIMARY KEY,
    economy_enabled  BOOLEAN,
    currency_name    TEXT,
    currency_emoji   TEXT,
    xp_per_message   BIGINT,
    xp_cooldown_secs INTEGER,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT currency_name_length CHECK (char_length(currency_name) BETWEEN 1 AND 32),
    CONSTRAINT currency_emoji_length CHECK (char_length(currency_emoji) BETWEEN 1 AND 32),
    CONSTRAINT xp_per_message_range CHECK (xp_per_message BETWEEN 1 AND 10000),
    CONSTRAINT xp_cooldown_range CHECK (xp_cooldown_secs BETWEEN 0 AND 86400)
);
