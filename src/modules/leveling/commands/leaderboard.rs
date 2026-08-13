use crate::{Context, error::AppError, modules::leveling::store};
use poise::serenity_prelude as serenity;

/// Show this server's top members by XP
#[poise::command(slash_command, guild_only)]
pub async fn leaderboard(ctx: Context<'_>) -> Result<(), AppError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".to_string()))?
        .get() as i64;

    let rows = store::leaderboard(&ctx.data().db, guild_id, 10).await?;

    let guild_icon = ctx.guild().and_then(|g| g.icon_url());

    let body = if rows.is_empty() {
        "No one has earned XP yet.".to_string()
    } else {
        rows.iter()
            .enumerate()
            .map(|(i, r)| format!("{}. <@{}> — {} XP", i + 1, r.user_id, r.experience))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut embed = serenity::CreateEmbed::new()
        .title("Leaderboard")
        .description(body);

    if let Some(icon) = guild_icon {
        embed = embed.thumbnail(icon);
    }

    ctx.send(poise::CreateReply::default().embed(embed)).await?;

    Ok(())
}
