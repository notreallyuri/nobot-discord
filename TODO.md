# Discord Bot — Feature TODO

Items that need database work carry a nested `db —` line naming the schema change.
Tick it when the migration is written and applied; the parent stays open until the
feature itself ships. Items with no nested line need no schema change.

## ✅ Already Implemented

- [x] Music streaming with queue controls
- [x] Spotify support
- [x] Profile / leveling system
- [x] Achievement system
- [x] Coin system with badge support
- [x] Shop system (badges purchasable with coins)
- [x] Profile customization — custom banners via Discord media upload
- [x] Image-generated profile cards (resvg + SVG, not canvas/Pillow)
- [x] Slash command autocomplete for song search

---

## ⚙️ Quality of Life / Infra

- [x] **Per-server config** — `/config show · economy · currency · xp · reset`,
      gated on Manage Server
  - [x] db — `0007_guild_config`, keyed by `guild_id`, one nullable column per
        setting so NULL means "fall back to the bot default"
  - Later features add their columns here with `ALTER TABLE`, the way `profile`
    grew. Currently carries: economy toggle, currency name/emoji, XP per message,
    XP cooldown.
- [ ] Web dashboard for admins (XP curve, coin rates, level roles)
  - [ ] db — session/auth storage; shape depends on the auth approach

## 🎵 Music

- [x] DJ role/permissions for skip/stop/queue control — `/config dj`
  - [x] db — `0008_dj_role`, `guild_config.dj_role_id`
  - Gates skip, stop, pause, resume, clear, remove, move, leave. Unset means
    anyone can control playback, which is the previous behaviour.
  - Server managers bypass it, and so does anyone who is the only listener in
    the channel — otherwise a DJ-less server locks itself out of its own bot.
- [x] More queue control — `/shuffle`, `/repeat off|track|queue`
  - Track repeat uses songbird's `enable_loop`, so the queue behind it is
    untouched and resumes the moment you switch modes.
  - Queue repeat re-enqueues a track when it ends. It only fires on a natural
    `PlayMode::End`, never on a stop, so `/skip`, `/stop` and `/clear` behave
    normally instead of refilling the queue.
  - Both are behind the DJ role.
- [x] 24/7 mode with auto-reconnect and idle timeout — `/config voice`
  - [x] db — `0009_voice_presence`: `stay_connected`, `idle_timeout_secs`, plus
        `voice_channel_id`/`voice_text_channel_id` as remembered state
  - Reconnect fires on `CacheReady`, skips guilds already connected, and logs
    rather than failing when the channel is gone or permissions were revoked.
  - `/leave` forgets the remembered channel, so being sent away sticks across a
    restart instead of the bot rejoining on its own.
- [ ] Saved/reloadable playlists per user
  - [ ] db — new `playlist` + `playlist_track` tables
- [x] Lyrics display — `/lyrics [song]`, via lrclib.net rather than
      Genius/Musixmatch: no API key, and it returns full lyrics. Genius's API
      does not return lyrics at all (only a URL to scrape) and Musixmatch's free
      tier caps at a 30% snippet.
  - lrclib also serves `syncedLyrics` in LRC format, which is what a
    follow-along/karaoke mode would need later. Not read yet.
- [ ] Cross-platform search (YouTube Music, SoundCloud)
- [ ] Audio filters/effects (bassboost, nightcore, speed) — runtime state only

## 🔗 Music + Profile Integration

- [ ] Coins/XP for listening time or song requests
  - [ ] db — counters on `profile`: `songs_requested`, `listening_seconds`
- [ ] "Music taste" badges (e.g. "Played 100 songs", "DJ of the week")
  - [ ] db — none of its own; reads the counters above
- [ ] Show "currently listening to" on profile card — reads live songbird state

## 🛡️ Moderation & Utility (optional expansion)

- [x] Welcome/leave messages with a generated card — `/config welcome`,
      `/config farewell`
  - [x] db — `0011_greetings`: welcome/farewell channel, message and card toggle
  - **Needs `MEMBER_INTENT=true` in `.env` and "Server Members Intent" enabled
    in the Discord developer portal.** Without it the bot stays on
    non-privileged intents and no member events arrive, so the settings save
    but nothing fires. The intent is opt-in precisely so an unset portal
    toggle cannot stop the bot connecting.
  - Templates take `{user}`, `{mention}`, `{server}`, `{count}`; separate
    channels mean welcomes and farewells can run independently.
- [ ] Logging (message edits/deletes, joins/leaves)
  - [ ] db — `guild_config.log_channel_id`
- [ ] Auto-moderation (spam/link filtering) — **scope not defined yet**
  - Needs both fixed toggles and user-defined rules, so it will want
    `guild_config` columns *and* an `automod_rule` table. Schema stays unwritten
    until the rule model is decided (what can be matched, what actions exist,
    how exemptions work).
- [x] Self-assignable roles — `/rolemenu create · add · remove · list · delete`
  - [x] db — `0010_role_menus`: `role_menu` + `role_menu_option`
  - Built on an embed with a select menu rather than reactions: labels and
    descriptions are visible, and deselecting removes the role.
  - The menu id lives in the component's `custom_id`, so posted menus keep
    working across restarts with no in-memory collector.
  - Adding or removing a role edits the posted message in place.

## 💰 Economy & Profile System

- [ ] Global leaderboard command — server-wide `/leaderboard` already ships, and
      `store::global_rank` is already written and used by the profile card
  - [x] db — indexes landed in `0006_ranking_indexes`
- [ ] Daily/weekly login streaks with bonus coins
  - [ ] db — `profile.streak`, `profile.last_claim_at`
- [ ] XP boosters in shop (temporary/permanent multipliers)
  - [ ] db — new `user_booster` table; temporary ones need an `expires_at`
- [ ] More shop customization options (profile colors, card themes, extra banner slots)
  - [ ] db — `profile.theme`, `profile.background_slots` (how many the user owns),
        and a `user_background` table, since `profile.background` holds exactly one
  - Two buttons under the profile message page through *sections*, up to 4 items
    per page.
  - **Grid: adaptive mosaic (decided).** The layout changes with the count so the
    frame is always full — 1 item fills it, 2 split it, 3 is one large plus two
    stacked, 4 is a 2×2. No empty cells at any count.
  - **Open: what the sections actually are.** The mosaic suits grid-shaped
    content, so it fits some candidates and not others. Drawn from this file:
    - Backgrounds — the saved-banner picker; the case that motivated all this
    - Badges — the card only ever shows the 6 equipped, and `/badges list` is a
      plain text embed, so a visual grid would be a real gain and can reuse
      `badges::render` as-is
    - Achievements — same argument; currently a text embed
    - Music stats — blocked on the listening counters above
    - Stats — the card as it exists today, which is *not* grid-shaped
  - The schema above does not depend on any of this, so the migration is not
    blocked — only the rendering and interaction work is.
  - Buttons are message components, not reactions. That matters: a component
    handler either lives for a bounded collector window, or encodes its state in
    the `custom_id` so it survives a bot restart. The second is what makes an old
    `/profile` message still work tomorrow.
- [ ] Prestige system (reset level at max for permanent perk/badge)
  - [ ] db — `profile.prestige`, global (decided)
  - Resetting is global too: one transaction zeroes `users.experience` *and*
    every one of that user's `guild_member.experience` rows. Destructive and
    irreversible, so it needs a confirmation step before it fires.
- [ ] Trading/gifting coins & badges between users
  - [ ] db — no new state strictly required, but a `transfer` ledger is worth it
        for reversing mistakes and catching abuse
- [ ] Gambling/minigames (slots, blackjack, coinflip) — spends existing coins

---

## Migration Order

`guild_config` first: it unblocks DJ roles, 24/7 mode, welcome messages, logging,
auto-moderation, and the "disable economy / custom currency" settings. Everything
else is independent and can land in any order.

| # | Migration | Unblocks |
|---|-----------|----------|
| ~~0007~~ | ~~`guild_config`~~ — done | 6 items across Music, Moderation, Economy |
| next | `playlist` + `playlist_track` | Saved playlists |
| next | `profile` listening counters | Music/profile integration, music badges |
| next | `profile` streak columns | Login streaks |
| next | `user_booster` | XP boosters |
| next | `user_background` + `profile.theme` | Extra banner slots, card themes |
| next | `transfer` ledger | Trading/gifting |

Only 0007 is numbered — later numbers get assigned when written, so parallel work
does not collide.

---

## Priority List (for development roadmap)
>
> QoL > Music > Profile Integration > Moderation > Economy

## Known Debt (not features)

- [ ] `/profile` runs six sequential queries that could be one round trip
- [ ] Section labels on the profile card use `accent.light`, which only guarantees
      luminance ≥ 0.12 — a very dark user accent reads at ~3:1 over a background
- [x] ~~Paused playback does not count toward the voice idle timeout~~ — fixed
      with 24/7: a paused track now counts as idle, and 24/7 mode or a longer
      `/config voice idle_timeout` is the way to hold the channel deliberately
