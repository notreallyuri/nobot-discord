-- Currency lives on the profile row alongside the other per-user state.
ALTER TABLE profile ADD COLUMN IF NOT EXISTS coins BIGINT NOT NULL DEFAULT 0;

-- Badge and achievement definitions live in code, not here: only ownership is
-- persisted, so the catalogue can change without a migration.
CREATE TABLE IF NOT EXISTS user_badge (
    user_id     BIGINT NOT NULL,
    badge_id    TEXT NOT NULL,
    equipped    BOOLEAN NOT NULL DEFAULT FALSE,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, badge_id)
);

CREATE TABLE IF NOT EXISTS user_achievement (
    user_id        BIGINT NOT NULL,
    achievement_id TEXT NOT NULL,
    unlocked_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, achievement_id)
);

-- The profile card only ever reads the equipped subset.
CREATE INDEX IF NOT EXISTS user_badge_equipped_idx
    ON user_badge (user_id) WHERE equipped;
