-- Restricts playback control to a role. NULL keeps the open behaviour where
-- anyone in the server can skip, stop or reorder the queue.

ALTER TABLE guild_config ADD COLUMN IF NOT EXISTS dj_role_id BIGINT;
