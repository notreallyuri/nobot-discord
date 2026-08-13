-- Self-assignable roles offered through a select menu rather than reactions.
--
-- The posted message is the source of truth for where a menu lives, so its ids
-- are stored here: the interaction handler only receives the menu id back in
-- the component's custom_id and has to find everything else from that.

CREATE TABLE IF NOT EXISTS role_menu (
    id          BIGSERIAL PRIMARY KEY,
    guild_id    BIGINT NOT NULL,
    channel_id  BIGINT NOT NULL,
    message_id  BIGINT,
    title       TEXT NOT NULL,
    description TEXT,
    min_choices SMALLINT NOT NULL DEFAULT 0,
    max_choices SMALLINT NOT NULL DEFAULT 25,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT role_menu_title_length CHECK (char_length(title) BETWEEN 1 AND 100),
    CONSTRAINT role_menu_choice_range CHECK (min_choices BETWEEN 0 AND 25
                                         AND max_choices BETWEEN 1 AND 25
                                         AND min_choices <= max_choices)
);

CREATE TABLE IF NOT EXISTS role_menu_option (
    menu_id     BIGINT NOT NULL REFERENCES role_menu (id) ON DELETE CASCADE,
    role_id     BIGINT NOT NULL,
    label       TEXT NOT NULL,
    description TEXT,
    position    INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (menu_id, role_id),
    CONSTRAINT role_menu_option_label_length CHECK (char_length(label) BETWEEN 1 AND 100),
    CONSTRAINT role_menu_option_desc_length CHECK (description IS NULL
                                               OR char_length(description) BETWEEN 1 AND 100)
);

CREATE INDEX IF NOT EXISTS role_menu_guild_idx ON role_menu (guild_id);
CREATE INDEX IF NOT EXISTS role_menu_option_menu_idx ON role_menu_option (menu_id, position);
