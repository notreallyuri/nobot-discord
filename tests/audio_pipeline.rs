use songbird::input::{
    Input, YoutubeDl,
    codecs::{get_codec_registry, get_probe},
};

const TRACK: &str = "https://www.youtube.com/watch?v=bMhDJ0S0OBA";

#[tokio::test]
#[ignore = "requires network access and yt-dlp"]
async fn previously_forbidden_track_still_plays() {
    let input: Input = YoutubeDl::new(
        reqwest::Client::new(),
        "https://www.youtube.com/watch?v=uiBiKC0TC3I",
    )
    .into();

    input
        .make_playable_async(get_codec_registry(), get_probe())
        .await
        .expect("public track should stream");
}

#[tokio::test]
#[ignore = "requires network access and yt-dlp"]
async fn youtube_search_audio_is_demuxable() {
    let input: Input =
        YoutubeDl::new_search(reqwest::Client::new(), "Daft Punk - Around the World").into();

    let live = input
        .make_playable_async(get_codec_registry(), get_probe())
        .await
        .expect("a searched stream should be fetchable and demuxable");

    assert!(matches!(live, Input::Live(..)));
}

#[tokio::test]
#[ignore = "requires network access and yt-dlp"]
async fn youtube_audio_is_demuxable() {
    let input: Input = YoutubeDl::new(reqwest::Client::new(), TRACK).into();

    let live = input
        .make_playable_async(get_codec_registry(), get_probe())
        .await
        .expect("yt-dlp stream should be demuxable by the enabled symphonia formats");

    assert!(
        matches!(live, Input::Live(..)),
        "input should be promoted to a live, parsed stream"
    );
}
