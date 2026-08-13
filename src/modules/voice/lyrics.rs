use crate::error::AppError;
use serde::Deserialize;
use std::time::Duration;

const API: &str = "https://lrclib.net/api";
const TIMEOUT: Duration = Duration::from_secs(6);
const AGENT: &str = concat!("dis-ru/", env!("CARGO_PKG_VERSION"));

pub const PAGE_CHARS: usize = 3_500;

const NOISE: &[&str] = &[
    "official",
    "video",
    "audio",
    "lyric",
    "lyrics",
    "visualizer",
    "visualiser",
    "hd",
    "hq",
    "4k",
    "8k",
    "mv",
    "m/v",
    "remaster",
    "remastered",
    "explicit",
    "full album",
    "color coded",
    "colour coded",
    "letra",
    "sub español",
];

const FEATURE_MARKERS: &[&str] = &[" feat. ", " featuring ", " feat ", " ft. ", " ft "];
const SEPARATORS: &[&str] = &[" - ", " – ", " — "];

const TRAILING_NOISE: &[&str] = &[
    "official music video",
    "official lyric video",
    "official visualizer",
    "official audio",
    "official video",
    "official teaser",
    "official mv",
    "official m/v",
    "music video",
    "lyric video",
    "m/v",
    "mv",
];

const QUOTE_PAIRS: &[(char, char)] = &[
    ('\u{2018}', '\u{2019}'),
    ('\u{201C}', '\u{201D}'),
    ('「', '」'),
    ('『', '』'),
    ('"', '"'),
];

#[derive(Debug, PartialEq, Eq)]
pub struct Search {
    pub artist: Option<String>,
    pub title: String,
    pub guessed: bool,
}

#[derive(Debug)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub instrumental: bool,
    pub lyrics: Option<String>,
}

fn strip_bracketed_noise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut group = String::new();
    let mut depth = 0usize;

    for c in text.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                if depth == 1 {
                    group.clear();
                } else {
                    group.push(c);
                }
            }
            ')' | ']' | '}' if depth > 0 => {
                depth -= 1;
                if depth > 0 {
                    group.push(c);
                } else {
                    let lowered = group.to_lowercase();
                    if !NOISE.iter().any(|noise| lowered.contains(noise)) {
                        out.push('(');
                        out.push_str(&group);
                        out.push(')');
                    }
                }
            }
            _ if depth == 0 => out.push(c),
            _ => group.push(c),
        }
    }

    if depth > 0 {
        out.push_str(&group);
    }

    out
}

fn strip_featuring(text: &str) -> String {
    let lowered = text.to_ascii_lowercase();

    FEATURE_MARKERS
        .iter()
        .filter_map(|marker| lowered.find(marker))
        .min()
        .map_or_else(|| text.to_string(), |at| text[..at].to_string())
}

fn trim_edges(text: &str) -> &str {
    text.trim_matches(|c: char| c == '-' || c == '|' || c == '·' || c.is_whitespace())
}

fn strip_trailing_noise(text: &str) -> String {
    let mut text = trim_edges(text).to_string();

    loop {
        let lowered = text.to_ascii_lowercase();

        let Some(noise) = TRAILING_NOISE
            .iter()
            .find(|noise| lowered.ends_with(*noise))
            .filter(|noise| noise.len() < text.len())
        else {
            return text;
        };

        let kept = trim_edges(&text[..text.len() - noise.len()]).to_string();

        if kept.is_empty() {
            return text;
        }

        text = kept;
    }
}

pub fn clean(text: &str) -> String {
    let stripped = strip_featuring(&strip_bracketed_noise(text));
    let collapsed = stripped.split_whitespace().collect::<Vec<_>>().join(" ");

    strip_trailing_noise(&collapsed)
}

fn strip_all_brackets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;

    for c in text.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn quoted(text: &str) -> Option<(String, String)> {
    for (open, close) in QUOTE_PAIRS {
        let Some(start) = text.find(*open) else {
            continue;
        };

        let after = start + open.len_utf8();
        let Some(offset) = text[after..].find(*close) else {
            continue;
        };

        let inside = text[after..after + offset].trim();

        if !inside.is_empty() {
            return Some((text[..start].to_string(), inside.to_string()));
        }
    }

    None
}

fn performer(text: &str) -> Option<String> {
    let name = trim_edges(&strip_all_brackets(text)).to_string();
    (!name.is_empty()).then_some(name)
}

pub fn split(raw: &str) -> Search {
    let cleaned = clean(raw);

    if let Some((before, inside)) = quoted(&cleaned) {
        let title = clean(&inside);

        if !title.is_empty() {
            return Search {
                artist: performer(&before),
                title,
                guessed: false,
            };
        }
    }

    for separator in SEPARATORS {
        if let Some((artist, title)) = cleaned.split_once(separator) {
            let (artist, title) = (artist.trim(), title.trim());

            if !artist.is_empty() && !title.is_empty() {
                return Search {
                    artist: Some(artist.to_string()),
                    title: title.to_string(),
                    guessed: true,
                };
            }
        }
    }

    Search {
        artist: None,
        title: cleaned,
        guessed: true,
    }
}

fn is_artist(side: &str, artist: &str) -> bool {
    let (side, artist) = (side.trim().to_lowercase(), artist.trim().to_lowercase());

    !side.is_empty()
        && !artist.is_empty()
        && (side == artist || side.contains(&artist) || artist.contains(&side))
}

fn song_side(cleaned: &str, artist: &str) -> String {
    for separator in SEPARATORS {
        if let Some((left, right)) = cleaned.split_once(separator) {
            let (left, right) = (left.trim(), right.trim());

            if left.is_empty() || right.is_empty() {
                continue;
            }

            if is_artist(right, artist) {
                return left.to_string();
            }

            if is_artist(left, artist) {
                return right.to_string();
            }
        }
    }

    cleaned.to_string()
}

pub fn for_track(artist: Option<&str>, title: &str) -> Search {
    let cleaned = clean(title);
    let parsed = split(&cleaned);

    let Some(known) = artist.map(str::trim).filter(|a| !a.is_empty()) else {
        return parsed;
    };

    if !parsed.guessed && parsed.artist.is_some() {
        return parsed;
    }

    let known = clean(known);

    Search {
        artist: Some(known.clone()),
        title: song_side(&cleaned, &known),
        guessed: false,
    }
}

pub fn paginate(lyrics: &str) -> Vec<String> {
    let mut pages = Vec::new();
    let mut page = String::new();

    for stanza in lyrics.split("\n\n") {
        for chunk in split_oversized(stanza) {
            let needed = chunk.chars().count();

            if !page.is_empty() && page.chars().count() + needed + 2 > PAGE_CHARS {
                pages.push(std::mem::take(&mut page));
            }

            if !page.is_empty() {
                page.push_str("\n\n");
            }
            page.push_str(&chunk);
        }
    }

    if !page.is_empty() {
        pages.push(page);
    }

    pages
}

fn split_oversized(stanza: &str) -> Vec<String> {
    if stanza.chars().count() <= PAGE_CHARS {
        return vec![stanza.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in stanza.lines() {
        if !current.is_empty() && current.chars().count() + line.chars().count() + 1 > PAGE_CHARS {
            chunks.push(std::mem::take(&mut current));
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiSong {
    track_name: String,
    artist_name: String,
    album_name: Option<String>,
    #[serde(default)]
    instrumental: bool,
    plain_lyrics: Option<String>,
}

fn tidy(raw: &str, artist: &str) -> String {
    let cleaned = clean(raw);
    let stripped = song_side(&cleaned, artist);

    if stripped.trim().is_empty() {
        cleaned
    } else {
        stripped.trim().to_string()
    }
}

impl From<ApiSong> for Song {
    fn from(api: ApiSong) -> Self {
        let artist = {
            let cleaned = clean(&api.artist_name);
            if cleaned.is_empty() {
                api.artist_name
            } else {
                cleaned
            }
        };

        let title = tidy(&api.track_name, &artist);

        let album = api
            .album_name
            .map(|album| tidy(&album, &artist))
            .filter(|album| !album.is_empty() && !album.eq_ignore_ascii_case(&title));

        Self {
            title,
            artist,
            album,
            instrumental: api.instrumental,
            lyrics: api
                .plain_lyrics
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty()),
        }
    }
}

async fn exact(http: &reqwest::Client, artist: &str, title: &str) -> Option<Song> {
    let response = http
        .get(format!("{API}/get"))
        .query(&[("artist_name", artist), ("track_name", title)])
        .header(reqwest::header::USER_AGENT, AGENT)
        .timeout(TIMEOUT)
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    response.json::<ApiSong>().await.ok().map(Song::from)
}

fn tokens(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

fn resembles(candidate: &str, wanted: &str) -> bool {
    let wanted = tokens(wanted);
    if wanted.is_empty() {
        return true;
    }

    let candidate = tokens(candidate);
    if candidate.is_empty() {
        return false;
    }

    let hits = wanted
        .iter()
        .filter(|word| candidate.contains(word))
        .count();

    hits * 2 >= wanted.len().min(candidate.len())
}

pub fn quality(song: &Song) -> u32 {
    let Some(lyrics) = &song.lyrics else {
        return 0;
    };

    let mut score = 0;

    if lyrics.contains("\n\n") {
        score += 4;
    }

    let lines: Vec<&str> = lyrics
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    if !lines.is_empty() {
        let average = lines.iter().map(|line| line.chars().count()).sum::<usize>() / lines.len();

        score += match average {
            0..=17 => 0,
            18..=24 => 1,
            _ => 2,
        };
    }

    score
}

async fn candidates(http: &reqwest::Client, query: &str) -> Vec<Song> {
    let Ok(response) = http
        .get(format!("{API}/search"))
        .query(&[("q", query)])
        .header(reqwest::header::USER_AGENT, AGENT)
        .timeout(TIMEOUT)
        .send()
        .await
    else {
        return Vec::new();
    };

    if !response.status().is_success() {
        return Vec::new();
    }

    response
        .json::<Vec<ApiSong>>()
        .await
        .map(|results| results.into_iter().map(Song::from).collect())
        .unwrap_or_default()
}

async fn best(
    http: &reqwest::Client,
    query: &str,
    wanted_title: &str,
    wanted_artist: Option<&str>,
) -> Option<Song> {
    let mut matches: Vec<Song> = candidates(http, query)
        .await
        .into_iter()
        .filter(|song| song.lyrics.is_some() || song.instrumental)
        .filter(|song| resembles(&song.title, wanted_title))
        .filter(|song| wanted_artist.is_none_or(|artist| is_artist(&song.artist, artist)))
        .collect();

    matches.sort_by_key(|song| std::cmp::Reverse(quality(song)));
    matches.into_iter().next()
}

pub async fn find(http: &reqwest::Client, search: &Search) -> Result<Option<Song>, AppError> {
    if search.title.is_empty() {
        return Err(AppError::Message(
            "There's nothing to search for — give me a song name.".into(),
        ));
    }

    if let Some(artist) = &search.artist {
        let combined = format!("{artist} {}", search.title);

        if let Some(song) = best(http, &combined, &search.title, Some(artist)).await {
            return Ok(Some(song));
        }

        if search.guessed
            && let Some(song) = best(http, &combined, artist, Some(&search.title)).await
        {
            return Ok(Some(song));
        }

        if let Some(song) = exact(http, artist, &search.title).await {
            return Ok(Some(song));
        }

        if search.guessed
            && let Some(song) = exact(http, &search.title, artist).await
        {
            return Ok(Some(song));
        }
    }

    Ok(best(http, &search.title, &search.title, None).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_usual_youtube_decoration() {
        let cases = [
            (
                "Daft Punk - Around the World (Official Video)",
                "Daft Punk - Around the World",
            ),
            (
                "Never Gonna Give You Up [Official Music Video]",
                "Never Gonna Give You Up",
            ),
            ("Some Song (Remastered 2011) [4K]", "Some Song"),
            ("Track Name (Lyrics)", "Track Name"),
            ("Song ft. Another Artist", "Song"),
            ("Song feat. Someone (Official Audio)", "Song"),
            ("Title (Live at Wembley)", "Title (Live at Wembley)"),
            ("Clean Title", "Clean Title"),
        ];

        for (raw, expected) in cases {
            assert_eq!(clean(raw), expected, "input: {raw:?}");
        }
    }

    #[test]
    fn keeps_meaningful_parentheses() {
        assert_eq!(
            clean("Everything In Its Right Place (Radio Edit)"),
            "Everything In Its Right Place (Radio Edit)"
        );
    }

    #[test]
    fn splits_artist_from_title() {
        assert_eq!(
            split("Daft Punk - Around the World (Official Video)"),
            Search {
                artist: Some("Daft Punk".into()),
                title: "Around the World".into(),
                guessed: true,
            }
        );

        assert_eq!(
            split("Just A Title"),
            Search {
                artist: None,
                title: "Just A Title".into(),
                guessed: true,
            }
        );
    }

    #[test]
    fn a_known_artist_picks_the_song_off_either_side() {
        let expected = |title: &str| Search {
            artist: Some("Linkin Park".into()),
            title: title.into(),
            guessed: false,
        };

        assert_eq!(
            for_track(
                Some("Linkin Park"),
                "Heavy Is the Crown (Official Audio) - Linkin Park"
            ),
            expected("Heavy Is the Crown"),
            "artist on the right"
        );

        assert_eq!(
            for_track(Some("Linkin Park"), "Linkin Park - Numb (Official Video)"),
            expected("Numb"),
            "artist on the left"
        );

        assert_eq!(
            for_track(Some("Linkin Park"), "Heavy Is the Crown"),
            expected("Heavy Is the Crown"),
            "no separator at all"
        );

        assert_eq!(
            for_track(
                Some("Linkin Park - Topic"),
                "Heavy Is the Crown - Linkin Park"
            ),
            Search {
                artist: Some("Linkin Park - Topic".into()),
                title: "Heavy Is the Crown".into(),
                guessed: false,
            },
            "a topic-channel artist should still match its side"
        );

        assert_eq!(
            for_track(Some("   "), "Artist - Song"),
            Search {
                artist: Some("Artist".into()),
                title: "Song".into(),
                guessed: true,
            },
            "a blank artist should fall back to parsing the title"
        );
    }

    #[test]
    fn bare_trailing_noise_is_stripped_too() {
        assert_eq!(clean("Magnetic Official MV"), "Magnetic");
        assert_eq!(clean("Some Song Official Music Video"), "Some Song");
        assert_eq!(clean("Track M/V"), "Track");
        assert_eq!(clean("Track - Official Video"), "Track");
        assert_eq!(
            clean("Make It Official"),
            "Make It Official",
            "a real word ending must survive"
        );
        assert_eq!(clean("MV"), "MV", "never strip the whole title away");
    }

    #[test]
    fn a_quoted_title_wins_over_everything_around_it() {
        assert_eq!(
            split("ILLIT (아일릿) \u{2018}Magnetic\u{2019} Official MV"),
            Search {
                artist: Some("ILLIT".into()),
                title: "Magnetic".into(),
                guessed: false,
            }
        );

        assert_eq!(
            split("NewJeans (뉴진스) \u{201C}Super Shy\u{201D} Official MV"),
            Search {
                artist: Some("NewJeans".into()),
                title: "Super Shy".into(),
                guessed: false,
            }
        );

        assert_eq!(
            split("YOASOBI「アイドル」Official Music Video"),
            Search {
                artist: Some("YOASOBI".into()),
                title: "アイドル".into(),
                guessed: false,
            }
        );
    }

    #[test]
    fn a_channel_name_does_not_override_the_titles_own_artist() {
        assert_eq!(
            for_track(
                Some("HYBE LABELS"),
                "ILLIT (아일릿) \u{2018}Magnetic\u{2019} Official MV"
            ),
            Search {
                artist: Some("ILLIT".into()),
                title: "Magnetic".into(),
                guessed: false,
            },
            "the uploader is a label, not the performer"
        );

        assert_eq!(
            for_track(Some("Daft Punk"), "Around the World (Official Video)"),
            Search {
                artist: Some("Daft Punk".into()),
                title: "Around the World".into(),
                guessed: false,
            },
            "an artist absent from the title is still right when the title names nobody"
        );
    }

    #[test]
    fn a_title_where_neither_side_is_the_artist_survives_intact() {
        assert_eq!(
            for_track(Some("Some Band"), "Alpha - Beta"),
            Search {
                artist: Some("Some Band".into()),
                title: "Alpha - Beta".into(),
                guessed: false,
            },
            "with neither side matching, do not guess which half to drop"
        );
    }

    fn song_with(lyrics: &str) -> Song {
        Song {
            title: "Heavy Is the Crown".into(),
            artist: "Linkin Park".into(),
            album: None,
            instrumental: false,
            lyrics: Some(lyrics.into()),
        }
    }

    #[test]
    fn stanza_breaks_and_full_lines_outrank_subtitle_fragments() {
        let readable = song_with(
            "One knock at the door and then we both know how the story ends\n\
             You can't win if your white flag's out when the war begins\n\
             \n\
             This is what you asked for, heavy is the crown\n\
             Fire in the sunrise, ashes rainin' down",
        );

        let fragments = song_with(
            "One knock at the door\nand then we both know\nhow the story ends\n\
             You can't win\nif your white flag's out\nwhen the war begins\n\
             This is what you\nasked for, heavy is\nthe crown",
        );

        assert!(
            quality(&readable) > quality(&fragments),
            "readable {} vs fragments {}",
            quality(&readable),
            quality(&fragments)
        );
    }

    #[test]
    fn messy_record_names_are_tidied_for_display() {
        let api = ApiSong {
            track_name: "Heavy Is the Crown  (Official Audio) - Linkin Park".into(),
            artist_name: "Linkin Park -".into(),
            album_name: Some("Heavy Is the Crown - Linkin Park".into()),
            instrumental: false,
            plain_lyrics: Some("a line".into()),
        };

        let song = Song::from(api);

        assert_eq!(song.artist, "Linkin Park");
        assert_eq!(song.title, "Heavy Is the Crown");
        assert_eq!(
            song.album, None,
            "an album that just repeats the title is noise"
        );
    }

    #[test]
    fn a_real_album_is_kept() {
        let api = ApiSong {
            track_name: "Around the World".into(),
            artist_name: "Daft Punk".into(),
            album_name: Some("Homework".into()),
            instrumental: false,
            plain_lyrics: Some("a line".into()),
        };

        assert_eq!(Song::from(api).album.as_deref(), Some("Homework"));
    }

    #[test]
    fn an_empty_record_scores_nothing() {
        let blank = Song {
            lyrics: None,
            ..song_with("")
        };

        assert_eq!(quality(&blank), 0);
    }

    #[test]
    fn relevance_rejects_an_unrelated_result() {
        assert!(resembles("Heavy Is the Crown", "Heavy Is the Crown"));
        assert!(resembles(
            "Heavy Is the Crown - Linkin Park",
            "Heavy Is the Crown"
        ));
        assert!(resembles("Numb", "Numb (Official Video)"));

        assert!(!resembles("Numb", "Heavy Is the Crown"));
        assert!(!resembles("Linkin Park - Numb", "Heavy Is the Crown"));
        assert!(!resembles("Papercut", "Bohemian Rhapsody"));
    }

    #[test]
    fn survives_unbalanced_and_empty_input() {
        assert_eq!(clean(""), "");
        assert_eq!(clean("   "), "");
        assert_eq!(clean("---"), "");

        assert_eq!(
            clean("Song (unclosed"),
            "Song unclosed",
            "an unclosed group keeps its words and drops the stray bracket"
        );

        for pathological in ["((((", "))))", "[[[", "(", "([{", "}])"] {
            let _ = clean(pathological);
        }

        assert!(
            clean("Real Title (((").contains("Real Title"),
            "an unbalanced bracket must never eat the title"
        );
    }

    #[test]
    fn non_ascii_titles_are_not_mangled() {
        assert_eq!(
            clean("日本語のタイトル (Official Video)"),
            "日本語のタイトル"
        );
        assert_eq!(
            split("宇多田ヒカル - First Love"),
            Search {
                artist: Some("宇多田ヒカル".into()),
                title: "First Love".into(),
                guessed: true,
            }
        );
    }

    #[test]
    fn short_lyrics_fit_on_one_page() {
        let pages = paginate("line one\nline two\n\nline three");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0], "line one\nline two\n\nline three");
    }

    #[test]
    fn long_lyrics_split_on_stanza_boundaries() {
        let stanza = "a line that is reasonably long\n".repeat(40);
        let lyrics = [stanza.trim_end(); 6].join("\n\n");

        let pages = paginate(&lyrics);

        assert!(pages.len() > 1, "expected a split, got {}", pages.len());
        for (i, page) in pages.iter().enumerate() {
            assert!(
                page.chars().count() <= PAGE_CHARS,
                "page {i} is {} chars",
                page.chars().count()
            );
        }
    }

    #[test]
    fn a_single_giant_stanza_is_still_split() {
        let lyrics = "one line\n".repeat(2_000);
        let pages = paginate(lyrics.trim_end());

        assert!(pages.len() > 1);
        assert!(pages.iter().all(|p| p.chars().count() <= PAGE_CHARS));
    }

    #[test]
    fn pagination_keeps_every_line() {
        let lyrics = (0..500)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");

        let rejoined = paginate(&lyrics).join("\n");
        for i in 0..500 {
            assert!(rejoined.contains(&format!("line {i}")), "lost line {i}");
        }
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn finds_a_real_song() {
        let http = reqwest::Client::new();
        let search = for_track(Some("Daft Punk"), "Around the World (Official Video)");

        let song = find(&http, &search)
            .await
            .expect("request should succeed")
            .expect("Daft Punk should be in the database");

        println!("{} — {} ({:?})", song.artist, song.title, song.album);
        assert_eq!(song.artist, "Daft Punk");
        assert!(song.lyrics.is_some_and(|l| l.contains("Around the world")));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn a_wrong_artist_falls_back_to_the_title_alone() {
        let http = reqwest::Client::new();

        let search = Search {
            artist: Some("Queen Official VEVO Topic".into()),
            title: "Bohemian Rhapsody".into(),
            guessed: false,
        };

        let song = find(&http, &search)
            .await
            .expect("request should succeed")
            .expect("a bad artist should not sink a good title");

        println!("recovered: {} — {}", song.artist, song.title);
        assert!(song.title.to_lowercase().contains("bohemian"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn a_title_first_upload_resolves_to_the_right_song() {
        let http = reqwest::Client::new();
        let search = for_track(
            Some("Linkin Park"),
            "Heavy Is the Crown (Official Audio) - Linkin Park",
        );

        assert_eq!(search.title, "Heavy Is the Crown");

        let song = find(&http, &search)
            .await
            .expect("request should succeed")
            .expect("the song is in the database");

        println!("{} — {} ({:?})", song.artist, song.title, song.album);

        assert!(
            song.title.to_lowercase().contains("heavy is the crown"),
            "got {:?} instead",
            song.title
        );
        assert!(
            !song
                .lyrics
                .as_deref()
                .unwrap_or_default()
                .contains("I've become so numb"),
            "resolved to Numb again"
        );

        let lyrics = song.lyrics.as_deref().unwrap_or_default();
        let lines: Vec<&str> = lyrics.lines().filter(|l| !l.trim().is_empty()).collect();
        let average = lines.iter().map(|l| l.chars().count()).sum::<usize>() / lines.len();

        println!(
            "stanza breaks: {}, lines: {}, average length: {average}",
            lyrics.matches("\n\n").count(),
            lines.len()
        );

        assert!(
            lyrics.contains("\n\n"),
            "picked a record with no stanza breaks"
        );
        assert!(
            average >= 25,
            "picked a subtitle-fragment record (average line {average} chars)"
        );
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn a_label_uploaded_mv_resolves_to_the_performer() {
        let http = reqwest::Client::new();
        let search = for_track(
            Some("HYBE LABELS"),
            "ILLIT (아일릿) \u{2018}Magnetic\u{2019} Official MV",
        );

        assert_eq!(search.artist.as_deref(), Some("ILLIT"));
        assert_eq!(search.title, "Magnetic");

        let song = find(&http, &search)
            .await
            .expect("request should succeed")
            .expect("ILLIT - Magnetic is in the database");

        println!("{} — {} ({:?})", song.artist, song.title, song.album);
        assert_eq!(song.artist, "ILLIT");
        assert!(song.title.to_lowercase().contains("magnetic"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn nonsense_returns_nothing_rather_than_erroring() {
        let http = reqwest::Client::new();
        let search = Search {
            artist: None,
            title: "zzzzqqqq not a real song 91237418".into(),
            guessed: false,
        };

        assert!(find(&http, &search).await.expect("no error").is_none());
    }
}
