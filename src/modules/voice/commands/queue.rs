use crate::{Context, error::AppError, modules::voice::setup};
use poise::serenity_prelude as serenity;

const PAGE: usize = 10;

#[poise::command(slash_command, guild_only)]
pub async fn queue(ctx: Context<'_>) -> Result<(), AppError> {
    let call = setup::current_call(ctx).await?;
    let tracks = {
        let call = call.lock().await;
        call.queue().current_queue()
    };

    let Some((current, upcoming)) = tracks.split_first() else {
        ctx.send(setup::safe_reply("The queue is empty.")).await?;
        return Ok(());
    };

    let current = current.data::<setup::TrackMeta>();
    let mut description = format!("**Now playing**\n{}\n", link(&current),);

    if !upcoming.is_empty() {
        description.push_str("\n**Up next**\n");
        for (i, handle) in upcoming.iter().take(PAGE).enumerate() {
            let meta = handle.data::<setup::TrackMeta>();
            description.push_str(&format!(
                "{}. {} · {} · <@{}>\n",
                i + 1,
                link(&meta),
                setup::fmt_len(meta.duration),
                meta.requester,
            ));
        }

        if upcoming.len() > PAGE {
            description.push_str(&format!("\n…and {} more.", upcoming.len() - PAGE));
        }
    }

    let total: std::time::Duration = tracks
        .iter()
        .filter_map(|h| h.data::<setup::TrackMeta>().duration)
        .sum();

    let embed = serenity::CreateEmbed::new()
        .title(format!("Queue · {} track(s)", tracks.len()))
        .description(description)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Total length: {}",
            setup::fmt_duration(total)
        )));

    ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}

fn link(meta: &setup::TrackMeta) -> String {
    match &meta.url {
        Some(url) => format!("[{}]({})", meta.title, url),
        None => meta.title.clone(),
    }
}
