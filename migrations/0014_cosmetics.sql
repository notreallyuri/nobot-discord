-- Cosmetics bought from the shop: gradient accents and card effects. Like
-- badges, the catalogue lives in code and only ownership is persisted, so
-- adding or repricing a cosmetic needs no migration.
CREATE TABLE IF NOT EXISTS user_cosmetic (
    user_id     BIGINT NOT NULL,
    cosmetic_id TEXT NOT NULL,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, cosmetic_id)
);

-- What is worn, a slot per kind rather than an `equipped` flag the way badges
-- do it: one accent and one effect at a time, so the column is that constraint.
-- `accent_cosmetic` overrides `accent` rather than replacing it, so taking a
-- gradient off uncovers the colour set with `/color set`.
ALTER TABLE profile ADD COLUMN IF NOT EXISTS accent_cosmetic TEXT;
ALTER TABLE profile ADD COLUMN IF NOT EXISTS card_effect     TEXT;
