-- Ranking and leaderboard reads were unindexed: every /profile counted the
-- whole `users` table, and the backfill's per-user subquery could not use
-- guild_member's primary key because guild_id leads it.

CREATE INDEX IF NOT EXISTS users_experience_idx
    ON users (experience DESC);

CREATE INDEX IF NOT EXISTS guild_member_user_idx
    ON guild_member (user_id);

CREATE INDEX IF NOT EXISTS guild_member_rank_idx
    ON guild_member (guild_id, experience DESC);
