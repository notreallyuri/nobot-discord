use crate::{
    HttpKey, card,
    data::{Data, MemberId},
    error::AppError,
    module::{EventFuture, Module},
};
use poise::serenity_prelude as serenity;
use std::time::Duration;

pub mod achievements;
pub mod backfill;
pub mod badges;
pub mod boosters;
pub mod commands;
pub mod setup;
pub mod store;
pub mod storefront;

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
            match event {
                serenity::FullEvent::Message { new_message } => {
                    on_message(ctx, data, new_message).await?;
                }
                serenity::FullEvent::InteractionCreate {
                    interaction: serenity::Interaction::Component(press),
                } if press.data.custom_id.starts_with(storefront::PREFIX) => {
                    if let Err(e) = browse_shop(ctx, data, press).await {
                        tracing::warn!(?e, user = %press.user.id, "shop interaction failed");
                    }
                }
                _ => {}
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

    let award = store::add_xp(&data.db, key, config.xp_award()).await?;
    let before = award.experience - award.granted;

    if let Some(level) = setup::xp::leveled_up(before, award.experience) {
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

async fn browse_shop(
    ctx: &serenity::Context,
    data: &Data,
    press: &serenity::ComponentInteraction,
) -> Result<(), AppError> {
    let Some(step) = storefront::parse(&press.data.custom_id) else {
        return Ok(());
    };

    let (aisle, page) = match step {
        storefront::Move::Page(aisle, page) => (aisle, page),
        storefront::Move::Pick => {
            let serenity::ComponentInteractionDataKind::StringSelect { values } = &press.data.kind
            else {
                return Ok(());
            };

            let Some(aisle) = values
                .first()
                .and_then(|slug| storefront::Aisle::from_slug(slug))
            else {
                return Ok(());
            };

            (aisle, 0)
        }
    };

    let guild_id = press.guild_id.map_or(0, |id| id.get() as i64);
    let settings = data.guild_config(guild_id).await;
    let wallet = storefront::wallet(&data.db, press.user.id.get() as i64).await?;

    // The aisle's shelf is already on the message unless the aisle just
    // changed. Pointing the embed at what Discord hosts keeps a page turn to
    // one JSON round trip instead of re-uploading the image every press.
    let name = storefront::shelf_name(aisle);
    let hosted = press
        .message
        .attachments
        .iter()
        .find(|attachment| attachment.filename == name)
        .map(|attachment| attachment.url.clone());

    let image = hosted
        .clone()
        .unwrap_or_else(|| storefront::attached(aisle));
    let mut response = serenity::CreateInteractionResponseMessage::new()
        .embed(storefront::embed(
            aisle,
            page,
            &wallet,
            settings.currency(),
            &image,
        ))
        .components(storefront::components(aisle, page));

    if hosted.is_none()
        && let Some(shelf) = storefront::shelf(aisle).await
    {
        response = response.add_file(shelf);
    }

    press
        .create_response(
            &ctx.http,
            serenity::CreateInteractionResponse::UpdateMessage(response),
        )
        .await?;

    Ok(())
}
