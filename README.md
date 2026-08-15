# dis-ru

A Discord bot in Rust: music playback, a leveling economy, and image-generated
profile cards. One process, one Postgres database, 46 slash commands.

Built on [poise]/[serenity] for the Discord side, [songbird] + [symphonia] for
audio, [sqlx] for the database, and [resvg] for card rendering.

[poise]: https://github.com/serenity-rs/poise
[serenity]: https://github.com/serenity-rs/serenity
[songbird]: https://github.com/serenity-rs/songbird
[symphonia]: https://github.com/pdeljanov/Symphonia
[sqlx]: https://github.com/launchbadge/sqlx
[resvg]: https://github.com/linebender/resvg

## What it does

**Music.** `/play` takes a link (YouTube, Spotify, SoundCloud, anything yt-dlp
resolves) or bare words to search for, with autocomplete on the search. Queue
control is `/queue`, `/skip`, `/pause`, `/resume`, `/stop`, `/clear`,
`/remove`, `/move`, `/shuffle`, `/repeat off|track|queue` and `/nowplaying`.
`/lyrics` pulls from lrclib.net, which needs no API key and returns whole
lyrics rather than a snippet. Spotify links are resolved to a searchable
title/artist and played from YouTube — Spotify does not stream to third
parties — and need a client id and secret to enable.

**Leveling and economy.** Messages earn XP on a cooldown (75 XP, 30s by
default, both configurable per server), and levels follow `xp = 100 × level²`.
Every XP tick also pays 10 coins, which buy things: 21 badges worn on the
profile card, 4 XP boosters that multiply what you earn for an hour or a day,
and 7 cosmetics — 5 gradient accents and 2 card effects. `/shop list` is a
paged, illustrated storefront; `/shop buy` takes an item from any aisle.
Alongside those are 5 tenure achievements, `/leaderboard`, and `/profile`.

**Profile cards.** `/profile` renders a 560×560 PNG through resvg rather than a
canvas library: the card is an SVG template, rasterised and supersampled in a
blocking task. Members customise it with `/color set` (one hex, highlight
derived), `/background set` (an uploaded image, normalised and blurred for the
scrim), `/badges equip`, and `/cosmetics equip`. Level-ups and welcome messages
render their own cards from the same pipeline.

**Roles and greetings.** `/rolemenu` builds self-assign menus that survive a
restart — the whole selection lives in the component's `custom_id`, so there is
no collector to lose. `/autorole` assigns roles on join. `/config welcome` and
`/config farewell` post announcements with an optional generated card.

**Per-server config.** `/config show · economy · currency · xp · dj · voice ·
welcome · farewell · reset`, gated on Manage Server. Every setting is a
nullable column, so unset means "use the bot default" rather than a copy of it.
`/config dj` gates playback control behind a role; `/config voice` sets 24/7
mode and the idle timeout.

## Running it

You need Rust (edition 2024, so 1.85+), Docker or a Postgres 15 instance, and a
bot token from the [Discord developer portal].

```sh
cp .env.example .env      # then fill in TOKEN
docker compose up -d      # Postgres on :5436
cargo run                 # applies migrations on startup
```

Set `GUILD_IDS` to a comma-separated list while developing — guild commands
register instantly, global ones take up to an hour to propagate.

yt-dlp is the only external binary, and the bot finds it on its own: it uses
`YTDLP_PATH` if set, else yt-dlp on `PATH`, else downloads a copy on first run
and verifies it against the published `SHA2-256SUMS`. No ffmpeg — symphonia
decodes in-process.

Two features need opting in. Spotify links want `SPOTIFY_CLIENT_ID` and
`SPOTIFY_CLIENT_SECRET` (both or neither; an app needs no review). Welcome and
farewell announcements want `MEMBER_INTENT=true` **and** "Server Members
Intent" enabled in the portal — without it `/config welcome` saves happily and
then nothing ever fires. `.env.example` documents the rest.

[Discord developer portal]: https://discord.com/developers/applications

## Development

```sh
cargo test          # 199 tests, no database or network required
cargo clippy --tests
cargo fmt
```

Queries are checked at compile time. The `.sqlx` cache is committed, so a build
works offline with `SQLX_OFFLINE=true`; after changing any query, regenerate it
against a live database or the next offline build fails:

```sh
cargo sqlx prepare -- --tests
```

Migrations are plain SQL in `migrations/`, applied on startup and numbered when
they are written. Once a migration has been applied, its file is immutable —
sqlx stores a SHA-384 of the bytes and refuses to start if it changes, even for
a comment.

Most tests are unit tests beside the code. The card tests are the interesting
ones: they rasterise real SVG and assert on pixels — that text stays legible
over a bright background, that an effect does not bleed into the middle of the
card, that a badge row is actually drawn. Several `#[ignore]`d tests dump PNGs
to `$CARD_DUMP` for eyeballing a design change:

```sh
CARD_DUMP=/tmp/cards cargo test -- --ignored a_card_wearing_each_cosmetic --nocapture
```

The integration tests in `tests/` hit the network and yt-dlp, so they are
ignored by default.

### Layout

```
src/
  main.rs        entrypoint: config, pool, migrations, framework, shutdown
  module.rs      the Module trait — commands, setup, event handling
  config.rs      environment config, with secrets redacted from Debug
  guild_config.rs  per-server settings, cached, NULL means "bot default"
  card/          SVG templates and the resvg pipeline
  modules/
    leveling/    XP, coins, badges, boosters, cosmetics, shop, profile
    voice/       playback, queue, sources, Spotify, lyrics, yt-dlp
    roles/       self-assign menus and autorole
    greetings/   welcome and farewell announcements
migrations/      numbered SQL, applied on startup
assets/          fonts and icons compiled into the binary
```

A module owns its commands, its event handling and its tables; `modules::all()`
is the only place they are wired together. Adding one is a `Module` impl and a
line there.

## Notes on data

Everything is keyed by Discord snowflake. Profiles, coins and badges are global
to the bot; XP is tracked both per guild and globally, which is why `/profile`
shows two ranks. Uploaded backgrounds are stored as bytes in Postgres alongside
a pre-blurred copy, so a card render never waits on a fetch.

## Licences

Icons in `assets/icons/` are [Lucide], ISC. Fonts are Figtree (OFL) and Noto
Sans (OFL), including the CJK subset, so names outside Latin still render.

[Lucide]: https://lucide.dev
