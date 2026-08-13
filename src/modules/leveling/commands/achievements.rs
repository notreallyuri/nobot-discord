use crate::{
    Context,
    error::AppError,
    modules::leveling::{achievements, badges, setup::xp, store},
};
use poise::serenity_prelude as serenity;

/// See which achievements you've unlocked
#[poise::command(slash_command, guild_only)]
pub async fn achievements(ctx: Context<'_>) -> Result<(), AppError> {
    let user_id = ctx.author().id.get() as i64;
    let db = &ctx.data().db;

    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))?
        .get() as i64;

    let standing = store::guild_rank(db, crate::data::MemberId { guild_id, user_id }).await?;
    let level = xp::level_for_xp(standing.experience);

    let unlocked = store::unlocked_achievements(db, user_id).await?;

    let lines: Vec<String> = achievements::ACHIEVEMENTS
        .iter()
        .map(|achievement| {
            let done = unlocked.iter().any(|id| id == achievement.id);
            let mark = if done { "✅" } else { "🔒" };

            let reward = match achievement.badge.and_then(badges::find) {
                Some(badge) => {
                    format!("{} coins + the **{}** badge", achievement.coins, badge.name)
                }
                None => format!("{} coins", achievement.coins),
            };

            let progress = if done {
                String::new()
            } else {
                format!(" — you're level {level}")
            };

            format!(
                "{mark} **{}** · {}\n{reward}{progress}",
                achievement.name, achievement.description
            )
        })
        .collect();

    let embed = serenity::CreateEmbed::new()
        .title("Achievements")
        .description(lines.join("\n\n"))
        .footer(serenity::CreateEmbedFooter::new(format!(
            "{} of {} unlocked",
            unlocked.len(),
            achievements::ACHIEVEMENTS.len()
        )));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}
