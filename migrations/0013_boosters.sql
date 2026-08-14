-- XP boosters bought from the shop. Global like `profile`, since coins are
-- global: a booster is bought once and applies wherever the member talks.
CREATE TABLE IF NOT EXISTS user_booster (
    id             BIGSERIAL PRIMARY KEY,
    user_id        BIGINT      NOT NULL,
    -- Percent of normal XP, so 200 is double. Integer to keep the multiplier
    -- exact through SQL; the floor is 101 so a "booster" always boosts.
    multiplier_pct INT         NOT NULL CHECK (multiplier_pct BETWEEN 101 AND 1000),
    -- NULL is permanent. Everything else expires.
    expires_at     TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every message tick asks the same question: what is this member's strongest
-- live booster. Ordering by multiplier lets that answer come off the index.
CREATE INDEX IF NOT EXISTS user_booster_live_idx
    ON user_booster (user_id, multiplier_pct DESC)
    INCLUDE (expires_at);
