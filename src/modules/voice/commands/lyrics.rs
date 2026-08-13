use crate::{
    Context,
    error::AppError,
    modules::voice::{
        lyrics::{self, Search, Song},
        setup,
    },
};
use poise::serenity_prelude as serenity;
use std::time::Duration;

const BROWSE_FOR: Duration = Duration::from_secs(300);

/// Show the lyrics for a song, or for what's playing
#[poise::command(slash_command, guild_only)]
pub async fn lyrics(
    ctx: Context<'_>,
    #[description = "A song to look up (defaults to what's playing)"] song: Option<String>,
) -> Result<(), AppError> {
    ctx.defer().await?;

    let search = match &song {
        Some(raw) => lyrics::split(raw),
        None => from_queue(ctx).await?,
    };

    let http = setup::http_client(ctx).await?;

    let Some(found) = lyrics::find(&http, &search).await? else {
        return Err(AppError::Message(format!(
            "Couldn't find lyrics for **{}**.",
            display(&search)
        )));
    };

    if found.instrumental {
        ctx.send(setup::safe_reply(format!(
            "**{}** by **{}** is instrumental — no lyrics to show.",
            found.title, found.artist
        )))
        .await?;
        return Ok(());
    }

    let Some(text) = &found.lyrics else {
        return Err(AppError::Message(format!(
            "Found **{}** by **{}**, but it has no lyrics on record.",
            found.title, found.artist
        )));
    };

    let pages = lyrics::paginate(text);
    browse(ctx, &found, &pages).await
}

async fn from_queue(ctx: Context<'_>) -> Result<Search, AppError> {
    let call = setup::current_call(ctx).await?;

    let current = {
        let call = call.lock().await;
        call.queue().current()
    };

    let Some(current) = current else {
        return Err(AppError::Message(
            "Nothing is playing — name a song instead.".into(),
        ));
    };

    let meta = current.data::<setup::TrackMeta>();
    Ok(lyrics::for_track(meta.artist.as_deref(), &meta.title))
}

fn display(search: &Search) -> String {
    match &search.artist {
        Some(artist) => format!("{artist} — {}", search.title),
        None => search.title.clone(),
    }
}

fn page(song: &Song, pages: &[String], at: usize) -> serenity::CreateEmbed {
    let mut embed = serenity::CreateEmbed::new()
        .title(&song.title)
        .author(serenity::CreateEmbedAuthor::new(&song.artist))
        .description(&pages[at]);

    if let Some(album) = &song.album {
        embed = embed.footer(serenity::CreateEmbedFooter::new(if pages.len() > 1 {
            format!("{album} · page {} of {}", at + 1, pages.len())
        } else {
            album.clone()
        }));
    } else if pages.len() > 1 {
        embed = embed.footer(serenity::CreateEmbedFooter::new(format!(
            "Page {} of {}",
            at + 1,
            pages.len()
        )));
    }

    embed
}

async fn browse(ctx: Context<'_>, song: &Song, pages: &[String]) -> Result<(), AppError> {
    let reply = poise::CreateReply::default()
        .embed(page(song, pages, 0))
        .allowed_mentions(serenity::CreateAllowedMentions::new());

    if pages.len() == 1 {
        ctx.send(reply).await?;
        return Ok(());
    }

    let id = ctx.id();
    let (back, forward) = (format!("{id}-back"), format!("{id}-forward"));

    let buttons = serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(&back)
            .emoji('◀')
            .style(serenity::ButtonStyle::Secondary),
        serenity::CreateButton::new(&forward)
            .emoji('▶')
            .style(serenity::ButtonStyle::Secondary),
    ]);

    ctx.send(reply.components(vec![buttons])).await?;

    let mut at = 0usize;

    while let Some(press) = serenity::collector::ComponentInteractionCollector::new(ctx)
        .filter(move |press| press.data.custom_id.starts_with(&id.to_string()))
        .timeout(BROWSE_FOR)
        .await
    {
        if press.data.custom_id == forward {
            at = (at + 1) % pages.len();
        } else if press.data.custom_id == back {
            at = at.checked_sub(1).unwrap_or(pages.len() - 1);
        } else {
            continue;
        }

        press
            .create_response(
                ctx.serenity_context(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new().embed(page(song, pages, at)),
                ),
            )
            .await?;
    }

    Ok(())
}
