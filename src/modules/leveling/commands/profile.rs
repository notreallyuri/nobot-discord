use crate::{
    Context, HttpKey, card,
    data::MemberId,
    error::AppError,
    modules::leveling::{badges, cosmetics, setup::xp, store},
};
use poise::serenity_prelude as serenity;

/// Show a member's profile card
#[poise::command(slash_command, guild_only)]
pub async fn profile(
    ctx: Context<'_>,
    #[description = "Whose profile to view (defaults to you)"] user: Option<serenity::User>,
) -> Result<(), AppError> {
    ctx.defer().await?;

    let target = user.as_ref().unwrap_or_else(|| ctx.author());

    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".to_string()))?
        .get() as i64;
    let user_id = target.id.get() as i64;
    let key = MemberId { guild_id, user_id };

    let data = ctx.data();
    let page = store::profile_page(&data.db, key).await?;

    let http = ctx
        .serenity_context()
        .data
        .read()
        .await
        .get::<HttpKey>()
        .cloned();
    let (avatar, images) = tokio::join!(
        async {
            match &http {
                Some(client) => card::avatar_data_uri(client, target).await,
                None => None,
            }
        },
        card::background::uris_for_card(page.background, page.background_blur),
    );

    let accent = cosmetics::accent(page.accent_cosmetic.as_deref(), page.accent);

    if let Some(restored) = &images.restore
        && let Err(error) =
            store::set_background(&data.db, user_id, &restored.sharp, &restored.blurred).await
    {
        tracing::warn!(?error, user_id, "could not store the refitted background");
    }

    let equipped: Vec<card::emblem::Emblem<'_>> = page
        .badges
        .iter()
        .filter_map(|id| badges::find(id))
        .map(|badge| card::emblem::Emblem {
            icon: badge.icon,
            colour: badge.colour,
        })
        .collect();
    let settings = data.guild_config(guild_id).await;

    let svg = card::profile::svg(&card::profile::Profile {
        name: target.display_name(),
        handle: &target.name,
        accent: &accent,
        avatar: avatar.as_deref(),
        background: images.sharp.as_deref(),
        background_blur: images.blurred.as_deref(),
        guild: standing(&page.guild),
        global: standing(&page.global),
        badges: &equipped,
        coins: page.coins,
        currency: settings.currency(),
        effect: cosmetics::effect(page.card_effect.as_deref()),
    });

    let png = card::render_async(
        svg,
        card::profile::WIDTH,
        card::profile::HEIGHT,
        card::SUPERSAMPLE,
    )
    .await?;

    ctx.send(
        poise::CreateReply::default()
            .attachment(serenity::CreateAttachment::bytes(png, "profile.png")),
    )
    .await?;

    Ok(())
}

fn standing(info: &store::RankInfo) -> card::profile::Standing {
    card::profile::Standing {
        level: xp::level_for_xp(info.experience),
        rank: info.rank,
        experience: info.experience,
        progress: xp::level_progress(info.experience),
    }
}
