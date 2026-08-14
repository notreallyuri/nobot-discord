use crate::{
    HttpKey,
    data::{Data, MemberId},
    error::AppError,
    module::{EventFuture, Module},
};
use poise::serenity_prelude as serenity;
use std::time::Duration;

pub mod achievements;
pub mod backfill;
pub mod badges;
pub mod card;
pub mod commands;
pub mod setup;
pub mod store;

pub struct LevelingModule;

impl Module for LevelingModule {
    fn name(&self) -> &'static str {
        "Leveling"
    }

    fn commands(&self) -> Vec<crate::Command> {
        vec![
            commands::leaderboard(),
            commands::profile(),
            commands::background(),
            commands::color(),
            commands::badges(),
            commands::shop(),
            commands::achievements(),
            commands::config(),
        ]
    }

    fn setup(&self, _ctx: poise::serenity_prelude::Context, data: crate::data::Data) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(300));
            loop {
                tick.tick().await;
                setup::cooldown::prune(&data.xp_cooldown);
            }
        });
    }

    fn handle_event<'a>(
        &'a self,
        ctx: &'a poise::serenity_prelude::Context,
        event: &'a poise::serenity_prelude::FullEvent,
        data: &'a crate::data::Data,
    ) -> EventFuture<'a> {
        Box::pin(async move {
            if let serenity::FullEvent::Message { new_message } = event {
                on_message(ctx, data, new_message).await?;
            }
            Ok(())
        })
    }
}

async fn on_message(
    ctx: &serenity::Context,
    data: &Data,
    msg: &serenity::Message,
) -> Result<(), AppError> {
    if msg.author.bot {
        return Ok(());
    }
    let Some(guild_id) = msg.guild_id else {
        return Ok(());
    };

    let key = MemberId {
        guild_id: guild_id.get() as i64,
        user_id: msg.author.id.get() as i64,
    };

    let config = data.guild_config(key.guild_id).await;
    if !config.economy() {
        return Ok(());
    }

    if !setup::cooldown::claim_xp_slot(&data.xp_cooldown, key, config.xp_cooldown()) {
        return Ok(());
    }

    let amount = config.xp_award();
    let after = store::add_xp(&data.db, key, amount).await?;

    if let Some(level) = setup::xp::leveled_up(after - amount, after) {
        let unlocked = award_achievements(data, msg.author.id.get() as i64, level)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(?e, user = %msg.author.id, "failed to award achievements");
                Vec::new()
            });

        if let Err(e) = announce_level_up(ctx, data, msg, level, &unlocked).await {
            tracing::warn!(?e, user = %msg.author.id, "failed to announce level up");
        }
    }

    Ok(())
}

async fn award_achievements(
    data: &Data,
    user_id: i64,
    level: i64,
) -> Result<Vec<&'static achievements::Achievement>, AppError> {
    let candidates: Vec<String> = achievements::earned_at(level)
        .map(|a| a.id.to_string())
        .collect();

    let award = store::award_achievements(&data.db, user_id, &candidates).await?;

    Ok(award.achievements().collect())
}

async fn announce_level_up(
    ctx: &serenity::Context,
    data: &Data,
    msg: &serenity::Message,
    level: i64,
    unlocked: &[&'static achievements::Achievement],
) -> Result<(), AppError> {
    let http = ctx.data.read().await.get::<HttpKey>().cloned();
    let avatar = match &http {
        Some(client) => card::avatar_data_uri(client, &msg.author).await,
        None => None,
    };

    let accent = card::accent::Accent::from_stored(
        store::accent(&data.db, msg.author.id.get() as i64).await?,
    );

    let svg = card::levelup::svg(&card::levelup::LevelUp {
        name: msg.author.display_name(),
        accent: &accent,
        avatar: avatar.as_deref(),
        from: level - 1,
        to: level,
    });

    let png = card::render_async(
        svg,
        card::levelup::WIDTH,
        card::levelup::HEIGHT,
        card::SUPERSAMPLE,
    )
    .await?;

    let mut content = format!("<@{}>", msg.author.id);
    for achievement in unlocked {
        content.push_str(&format!(
            "\n🏆 **{}** unlocked — +{} coins",
            achievement.name, achievement.coins
        ));
    }

    let message = serenity::CreateMessage::new()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new().users([msg.author.id]))
        .add_file(serenity::CreateAttachment::bytes(png, "levelup.png"));

    msg.channel_id.send_message(&ctx.http, message).await?;
    Ok(())
}
