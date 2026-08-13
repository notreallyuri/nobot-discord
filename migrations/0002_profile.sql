-- Per-user profile customisation. One row per user, columns added as new
-- options appear, rather than a table per setting.
--
-- `background` holds an image already decoded, cropped and re-encoded by the
-- bot, so the row size is bounded and rendering never re-fetches from Discord.
-- `accent` is 0xRRGGBB. Both are null until the user sets them.
CREATE TABLE IF NOT EXISTS profile (
    user_id     BIGINT NOT NULL PRIMARY KEY,
    background  BYTEA,
    accent      INTEGER,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
