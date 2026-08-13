CREATE TABLE IF NOT EXISTS users (
  user_id         BIGINT NOT NULL PRIMARY KEY,
  experience      BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS guild_member (
    user_id       BIGINT NOT NULL,
    guild_id      BIGINT NOT NULL,
    experience    BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (guild_id, user_id)
);
