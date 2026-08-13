use crate::error::AppError;
use ::serenity::prelude::TypeMapKey;
use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use reqwest::Client as HttpClient;
use songbird::SerenityInit;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

pub mod config;
pub mod data;
pub mod error;
pub mod guild_config;
pub mod module;
pub mod modules;

pub type Context<'a> = poise::Context<'a, data::Data, AppError>;
pub type Command = poise::Command<data::Data, AppError>;

pub struct HttpKey;

impl TypeMapKey for HttpKey {
    type Value = HttpClient;
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = Arc::new(config::Config::from_env()?);
    let token = config.token.clone();

    let mut intents = serenity::GatewayIntents::non_privileged();
    if config.member_intent {
        intents |= serenity::GatewayIntents::GUILD_MEMBERS;
        tracing::info!("member events enabled (needs Server Members Intent in the portal)");
    }

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(config.database_url.as_str())
        .await?;
    sqlx::migrate!().run(&db).await?;
    tracing::info!("migrations applied");

    if let Err(e) = modules::leveling::backfill::run(&db).await {
        tracing::error!(?e, "profile backfill failed, starting without it");
    }

    let modules = Arc::new(modules::all());
    let commands: Vec<Command> = modules.iter().flat_map(|m| m.commands()).collect();

    let http = HttpClient::new();
    modules::voice::ytdlp::resolve(&http).await;

    let spotify = config.spotify.clone().map(|creds| {
        tracing::info!("Spotify link support enabled");
        Arc::new(modules::voice::spotify::SpotifyClient::new(
            http.clone(),
            creds,
        ))
    });

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands,
            on_error: |error| Box::pin(handle_error(error)),
            event_handler: |ctx, event, _framework, data| {
                Box::pin(dispatch_event(ctx, event, data))
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                if config.guild_ids.is_empty() {
                    tracing::info!("registering commands globally (may take up to an hour)");
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                } else {
                    for guild_id in &config.guild_ids {
                        tracing::info!(%guild_id, "registering commands in guild");
                        poise::builtins::register_in_guild(
                            ctx,
                            &framework.options().commands,
                            *guild_id,
                        )
                        .await?;
                    }
                }

                let data = data::Data {
                    db,
                    config: config.clone(),
                    xp_cooldown: Arc::new(DashMap::new()),
                    modules: modules.clone(),
                    spotify,
                    voice_sessions: Arc::new(DashMap::new()),
                    guild_config: Arc::new(DashMap::new()),
                    repeat: Arc::new(DashMap::new()),
                };

                for module in modules.iter() {
                    tracing::info!(module = module.name(), "initialising module");
                    module.setup(ctx.clone(), data.clone());
                }

                Ok(data)
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .register_songbird()
        .type_map_insert::<HttpKey>(http)
        .await?;

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("shutdown signal received, closing shards");
        shard_manager.shutdown_all().await;
    });

    client.start().await?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,dis_ru=debug"));
    fmt().with_env_filter(filter).init();
}

async fn handle_error(error: poise::FrameworkError<'_, data::Data, AppError>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => {
            tracing::error!(?error, "failed during setup");
        }
        poise::FrameworkError::Command {
            error: AppError::Message(msg),
            ctx,
            ..
        } => {
            let _ = ctx
                .send(
                    poise::CreateReply::default()
                        .content(msg)
                        .allowed_mentions(serenity::CreateAllowedMentions::new())
                        .ephemeral(true),
                )
                .await;
        }
        poise::FrameworkError::Command { error, ctx, .. } => {
            tracing::error!(command = %ctx.command().name, ?error, "command returned an error");
            let _ = ctx
                .send(
                    poise::CreateReply::default()
                        .content("Something went wrong running that command.")
                        .ephemeral(true),
                )
                .await;
        }
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                tracing::error!(?e, "error while handling a framework error");
            }
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn dispatch_event(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: &data::Data,
) -> Result<(), AppError> {
    for module in data.modules.iter() {
        if let Err(e) = module.handle_event(ctx, event, data).await {
            tracing::error!(
                module = module.name(),
                ?e,
                "module failed to handle an event"
            );
        }
    }
    Ok(())
}
