use crate::{
    data::Data,
    error::AppError,
    module::{EventFuture, Module},
};
use poise::serenity_prelude as serenity;

pub mod commands;
pub mod menu;
pub mod store;

pub struct RolesModule;

impl Module for RolesModule {
    fn name(&self) -> &'static str {
        "Roles"
    }

    fn commands(&self) -> Vec<crate::Command> {
        vec![commands::rolemenu(), commands::autorole()]
    }

    fn handle_event<'a>(
        &'a self,
        ctx: &'a serenity::Context,
        event: &'a serenity::FullEvent,
        data: &'a Data,
    ) -> EventFuture<'a> {
        Box::pin(async move {
            match event {
                serenity::FullEvent::GuildMemberAddition { new_member } => {
                    grant_on_join(ctx, data, new_member).await;
                    return Ok(());
                }
                serenity::FullEvent::GuildMemberUpdate {
                    old_if_available,
                    new: Some(member),
                    ..
                } => {
                    let was_pending = old_if_available.as_ref().is_none_or(|old| old.pending);

                    if was_pending && !member.pending {
                        grant_on_join(ctx, data, member).await;
                    }

                    return Ok(());
                }
                _ => {}
            }

            let serenity::FullEvent::InteractionCreate {
                interaction: serenity::Interaction::Component(press),
            } = event
            else {
                return Ok(());
            };

            let Some(menu_id) = menu::menu_id_from(&press.data.custom_id) else {
                return Ok(());
            };

            if let Err(e) = apply(ctx, data, press, menu_id).await {
                tracing::warn!(?e, menu_id, "role menu interaction failed");
                let _ = reply(ctx, press, "Something went wrong applying those roles.").await;
            }

            Ok(())
        })
    }
}

async fn grant_on_join(ctx: &serenity::Context, data: &Data, member: &serenity::Member) {
    if member.user.bot || member.pending {
        return;
    }

    let guild_id = member.guild_id;
    let wanted = data.guild_config(guild_id.get() as i64).await.autoroles();

    if wanted.is_empty() {
        return;
    }

    let ceiling = assignable_below(ctx, guild_id);

    for role in wanted {
        if member.roles.contains(&role) {
            continue;
        }

        if ceiling.is_some_and(|highest| role_position(ctx, guild_id, role) >= highest) {
            tracing::warn!(
                %guild_id,
                %role,
                "skipping autorole: my highest role isn't above it"
            );
            continue;
        }

        if let Err(e) = member.add_role(&ctx.http, role).await {
            tracing::warn!(?e, %guild_id, %role, user = %member.user.id, "couldn't apply autorole");
        }
    }
}

async fn reply(
    ctx: &serenity::Context,
    press: &serenity::ComponentInteraction,
    body: &str,
) -> Result<(), AppError> {
    press
        .create_response(
            &ctx.http,
            serenity::CreateInteractionResponse::Message(
                serenity::CreateInteractionResponseMessage::new()
                    .content(body)
                    .allowed_mentions(serenity::CreateAllowedMentions::new())
                    .ephemeral(true),
            ),
        )
        .await?;

    Ok(())
}

async fn apply(
    ctx: &serenity::Context,
    data: &Data,
    press: &serenity::ComponentInteraction,
    menu_id: i64,
) -> Result<(), AppError> {
    let serenity::ComponentInteractionDataKind::StringSelect { values } = &press.data.kind else {
        return Ok(());
    };

    let Some(guild_id) = press.guild_id else {
        return Ok(());
    };

    let Some(record) = store::by_id(&data.db, menu_id).await? else {
        return reply(ctx, press, "This menu no longer exists.").await;
    };

    if record.guild_id != guild_id.get() as i64 {
        return reply(ctx, press, "This menu belongs to another server.").await;
    }

    let offered: Vec<serenity::RoleId> = store::choices(&data.db, menu_id)
        .await?
        .into_iter()
        .map(|choice| serenity::RoleId::new(choice.role_id as u64))
        .collect();

    let chosen: Vec<serenity::RoleId> = values
        .iter()
        .filter_map(|value| value.parse::<u64>().ok())
        .map(serenity::RoleId::new)
        .collect();

    let Some(member) = &press.member else {
        return Ok(());
    };

    let (add, remove) = menu::plan(&offered, &chosen, &member.roles);
    let ceiling = assignable_below(ctx, guild_id);

    let allowed = |role: &serenity::RoleId| {
        ceiling.is_none_or(|highest| role_position(ctx, guild_id, *role) < highest)
    };

    let blocked: Vec<serenity::RoleId> = add
        .iter()
        .chain(remove.iter())
        .filter(|role| !allowed(role))
        .copied()
        .collect();

    let mut applied = menu::Applied {
        added: Vec::new(),
        removed: Vec::new(),
        blocked,
    };

    for role in add.into_iter().filter(allowed) {
        match member.add_role(&ctx.http, role).await {
            Ok(()) => applied.added.push(role),
            Err(e) => {
                tracing::warn!(?e, %role, "couldn't add role");
                applied.blocked.push(role);
            }
        }
    }

    for role in remove.into_iter().filter(allowed) {
        match member.remove_role(&ctx.http, role).await {
            Ok(()) => applied.removed.push(role),
            Err(e) => {
                tracing::warn!(?e, %role, "couldn't remove role");
                applied.blocked.push(role);
            }
        }
    }

    reply(ctx, press, &applied.summary()).await
}

fn role_position(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    role: serenity::RoleId,
) -> u16 {
    ctx.cache
        .guild(guild_id)
        .and_then(|guild| guild.roles.get(&role).map(|role| role.position))
        .unwrap_or(u16::MAX)
}

fn assignable_below(ctx: &serenity::Context, guild_id: serenity::GuildId) -> Option<u16> {
    let guild = ctx.cache.guild(guild_id)?;
    let me = ctx.cache.current_user().id;
    let member = guild.members.get(&me)?;

    member
        .roles
        .iter()
        .filter_map(|role| guild.roles.get(role))
        .map(|role| role.position)
        .max()
}
