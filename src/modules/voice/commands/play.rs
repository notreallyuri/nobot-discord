use crate::{
    Context,
    error::AppError,
    modules::voice::{
        announce,
        setup::{self, TrackMeta},
        sources::{DeferredSearch, Query, Retrying},
        spotify::Resolution,
        suggest, ytdlp,
    },
};
use poise::serenity_prelude as serenity;
use songbird::{
    Call,
    input::{Compose, Input, YoutubeDl},
    tracks::Track,
};
use std::sync::Arc;
use tokio::sync::Mutex;

async fn suggestions<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let partial = partial.trim();

    if partial.is_empty() || !Query::is_search(partial) {
        return Vec::new();
    }

    let Ok(http) = setup::http_client(ctx).await else {
        return Vec::new();
    };

    let mut choices = vec![suggest::clip(partial)];

    for suggestion in suggest::youtube(&http, partial).await {
        if choices.len() >= suggest::MAX_CHOICES {
            break;
        }

        if !choices
            .iter()
            .any(|chosen| chosen.eq_ignore_ascii_case(&suggestion))
        {
            choices.push(suggestion);
        }
    }

    choices
}

#[poise::command(slash_command, guild_only)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "A link (YouTube, Spotify, SoundCloud…) or words to search for"]
    #[autocomplete = "suggestions"]
    query: String,
) -> Result<(), AppError> {
    ctx.defer().await?;

    let call = setup::join_or_get(ctx).await?;
    let http = setup::http_client(ctx).await?;

    let embed = match Query::classify(&query) {
        Query::Url(url) => {
            single(
                ctx,
                &call,
                YoutubeDl::new_ytdl_like(ytdlp::program(), http, url),
            )
            .await?
        }
        Query::Search(terms) => {
            single(
                ctx,
                &call,
                YoutubeDl::new_search_ytdl_like(ytdlp::program(), http, terms),
            )
            .await?
        }
        Query::Spotify(reference) => {
            let client = ctx.data().spotify.clone().ok_or_else(|| {
                AppError::Message(
                    "Spotify links aren't set up on this bot — ask the owner to configure \
                     SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET."
                        .into(),
                )
            })?;

            let limit = ctx.data().config.playlist_limit;
            let resolution = client.resolve(&reference, limit).await?;
            spotify(ctx, &call, http, reference.kind(), resolution).await?
        }
    };

    ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}

async fn single(
    ctx: Context<'_>,
    call: &Arc<Mutex<Call>>,
    mut source: YoutubeDl<'static>,
) -> Result<serenity::CreateEmbed, AppError> {
    let aux = source.aux_metadata().await.map_err(|e| {
        tracing::warn!(?e, "yt-dlp lookup failed");
        AppError::Message("Couldn't find anything for that.".into())
    })?;

    let meta = Arc::new(TrackMeta {
        title: aux.title.unwrap_or_else(|| "Unknown track".to_string()),
        artist: aux.artist,
        url: aux.source_url,
        duration: aux.duration,
        thumbnail: aux.thumbnail,
        requester: ctx.author().id,
    });

    let input = Input::Lazy(Box::new(Retrying::new(source)));
    let position = enqueue(ctx, call, input, meta.clone()).await;

    let footer = if position <= 1 {
        "Starting now".to_string()
    } else {
        format!("Position #{}", position - 1)
    };
    let heading = "Added to queue";

    let mut embed = serenity::CreateEmbed::new()
        .title(heading)
        .description(describe(&meta))
        .field("Length", setup::fmt_len(meta.duration), true)
        .field("Requested by", format!("<@{}>", meta.requester), true)
        .footer(serenity::CreateEmbedFooter::new(footer));

    if let Some(thumb) = &meta.thumbnail {
        embed = embed.thumbnail(thumb);
    }

    Ok(embed)
}

async fn spotify(
    ctx: Context<'_>,
    call: &Arc<Mutex<Call>>,
    http: reqwest::Client,
    kind: &str,
    resolution: Resolution,
) -> Result<serenity::CreateEmbed, AppError> {
    if resolution.tracks.is_empty() {
        return Err(AppError::Message(
            "That Spotify link has no playable tracks.".into(),
        ));
    }

    let requester = ctx.author().id;
    let mut first: Option<Arc<TrackMeta>> = None;
    let mut position = 0;
    let total = resolution.tracks.len();

    for track in &resolution.tracks {
        let meta = Arc::new(TrackMeta {
            title: track.search_query(),
            artist: (!track.artists.is_empty()).then(|| track.artists.join(", ")),
            url: track.url.clone(),
            duration: track.duration,
            thumbnail: track.art.clone().or_else(|| resolution.art.clone()),
            requester,
        });

        let input = Input::Lazy(Box::new(Retrying::new(DeferredSearch::new(
            http.clone(),
            track,
        ))));
        let at = enqueue(ctx, call, input, meta.clone()).await;

        if first.is_none() {
            first = Some(meta);
            position = at;
        }
    }

    let first = first.expect("non-empty checked above");

    let Some(collection) = resolution.collection else {
        let footer = if position <= 1 {
            "Starting now".to_string()
        } else {
            format!("Position #{}", position - 1)
        };

        let mut embed = serenity::CreateEmbed::new()
            .title("Added to queue")
            .description(describe(&first))
            .field("Length", setup::fmt_len(first.duration), true)
            .field("Requested by", format!("<@{requester}>"), true)
            .footer(serenity::CreateEmbedFooter::new(footer));

        if let Some(thumb) = &first.thumbnail {
            embed = embed.thumbnail(thumb);
        }

        return Ok(embed);
    };

    let mut footer = format!("Requested by {}", ctx.author().name);
    if resolution.truncated {
        footer.push_str(&format!(
            " · limited to the first {total} tracks of this {kind}"
        ));
    }

    let mut embed = serenity::CreateEmbed::new()
        .title(format!("Added {total} track(s)"))
        .description(format!("From the {kind} **{collection}**"))
        .field("Up first", describe(&first), false)
        .footer(serenity::CreateEmbedFooter::new(footer));

    if let Some(art) = &resolution.art {
        embed = embed.thumbnail(art);
    }

    Ok(embed)
}

async fn enqueue(
    ctx: Context<'_>,
    call: &Arc<Mutex<Call>>,
    input: Input,
    meta: Arc<TrackMeta>,
) -> usize {
    let mut call = call.lock().await;
    let handle = call
        .enqueue(Track::new_with_data(input, meta.clone()))
        .await;

    announce::attach(
        &handle,
        ctx.serenity_context().http.clone(),
        ctx.channel_id(),
        meta,
    );

    call.queue().len()
}

fn describe(meta: &TrackMeta) -> String {
    match &meta.url {
        Some(url) => format!("[{}]({})", meta.title, url),
        None => meta.title.clone(),
    }
}
