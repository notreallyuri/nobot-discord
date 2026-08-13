use crate::{
    Context, HttpKey,
    data::MemberId,
    error::AppError,
    modules::leveling::{
        badges,
        card::{self, accent::Accent},
        setup::xp,
        store,
    },
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
    let guild = store::guild_rank(&data.db, key).await?;
    let global = store::global_rank(&data.db, user_id).await?;

    let http = ctx
        .serenity_context()
        .data
        .read()
        .await
        .get::<HttpKey>()
        .cloned();
    let avatar = match &http {
        Some(client) => card::avatar_data_uri(client, target).await,
        None => None,
    };

    let style = store::profile_style(&data.db, user_id).await?;
    let (background, background_blur) =
        card::background::uris_for_card(style.background, style.background_blur).await;
    let accent = Accent::from_stored(style.accent);

    let equipped: Vec<&'static badges::Badge> = store::equipped_badges(&data.db, user_id)
        .await?
        .iter()
        .filter_map(|id| badges::find(id))
        .collect();
    let coins = store::balance(&data.db, user_id).await?;
    let settings = data.guild_config(guild_id).await;

    let svg = card::profile::svg(&card::profile::Profile {
        name: target.display_name(),
        accent: &accent,
        avatar: avatar.as_deref(),
        background: background.as_deref(),
        background_blur: background_blur.as_deref(),
        guild: standing(&guild),
        global: standing(&global),
        badges: &equipped,
        coins,
        currency: settings.currency(),
    });

    let png = card::render_async(svg, card::profile::WIDTH, card::profile::HEIGHT).await?;

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
