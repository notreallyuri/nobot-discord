use crate::{
    HttpKey, card,
    data::Data,
    guild_config::GuildConfig,
    module::{EventFuture, Module},
};
use poise::serenity_prelude as serenity;

pub mod template;

pub struct GreetingsModule;

impl Module for GreetingsModule {
    fn name(&self) -> &'static str {
        "Greetings"
    }

    fn commands(&self) -> Vec<crate::Command> {
        Vec::new()
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
                    greet(ctx, data, new_member.guild_id, &new_member.user, false).await;
                }
                serenity::FullEvent::GuildMemberRemoval { guild_id, user, .. } => {
                    greet(ctx, data, *guild_id, user, true).await;
                }
                _ => {}
            }

            Ok(())
        })
    }
}

fn destination(config: &GuildConfig, leaving: bool) -> Option<serenity::ChannelId> {
    let id = if leaving {
        config.farewell_channel_id
    } else {
        config.welcome_channel_id
    };

    id.filter(|id| *id > 0)
        .map(|id| serenity::ChannelId::new(id as u64))
}

async fn greet(
    ctx: &serenity::Context,
    data: &Data,
    guild_id: serenity::GuildId,
    user: &serenity::User,
    leaving: bool,
) {
    if user.bot {
        return;
    }

    let config = data.guild_config(guild_id.get() as i64).await;

    let Some(channel) = destination(&config, leaving) else {
        return;
    };

    let (server, members) = ctx
        .cache
        .guild(guild_id)
        .map(|guild| (guild.name.to_string(), guild.member_count))
        .unwrap_or_else(|| ("this server".to_string(), 0));

    let source = if leaving {
        config
            .farewell_message
            .as_deref()
            .unwrap_or(template::DEFAULT_FAREWELL)
    } else {
        config
            .welcome_message
            .as_deref()
            .unwrap_or(template::DEFAULT_WELCOME)
    };

    let body = template::render(
        source,
        &template::Fields {
            user: user.display_name(),
            mention: format!("<@{}>", user.id),
            server: &server,
            count: members,
        },
    );

    let mentions = if leaving {
        serenity::CreateAllowedMentions::new()
    } else {
        serenity::CreateAllowedMentions::new().users([user.id])
    };

    let mut message = serenity::CreateMessage::new()
        .content(body)
        .allowed_mentions(mentions);

    if config.shows_welcome_card()
        && let Some(png) = render_card(ctx, data, user, &server, members, leaving).await
    {
        message = message.add_file(serenity::CreateAttachment::bytes(png, "greeting.png"));
    }

    if let Err(e) = channel.send_message(&ctx.http, message).await {
        tracing::warn!(?e, %guild_id, %channel, leaving, "couldn't post the greeting");
    }
}

async fn render_card(
    ctx: &serenity::Context,
    data: &Data,
    user: &serenity::User,
    server: &str,
    members: u64,
    leaving: bool,
) -> Option<Vec<u8>> {
    let http = ctx.data.read().await.get::<HttpKey>().cloned();

    let avatar = match &http {
        Some(client) => card::avatar_data_uri(client, user).await,
        None => None,
    };

    let accent = crate::modules::leveling::store::accent(&data.db, user.id.get() as i64)
        .await
        .unwrap_or_default()
        .resolve();

    let svg = card::welcome::svg(&card::welcome::Welcome {
        name: user.display_name(),
        server,
        accent: &accent,
        avatar: avatar.as_deref(),
        member_number: members,
        leaving,
    });

    match card::render_async(
        svg,
        card::welcome::WIDTH,
        card::welcome::HEIGHT,
        card::SUPERSAMPLE,
    )
    .await
    {
        Ok(png) => Some(png),
        Err(e) => {
            tracing::warn!(?e, "couldn't render the greeting card");
            None
        }
    }
}
