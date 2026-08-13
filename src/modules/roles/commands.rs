use super::{menu, store};
use crate::{Context, error::AppError};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    guild_only,
    subcommands("create", "add", "remove", "list", "delete"),
    default_member_permissions = "MANAGE_ROLES",
    required_permissions = "MANAGE_ROLES"
)]
pub async fn rolemenu(_: Context<'_>) -> Result<(), AppError> {
    Ok(())
}

fn guild_of(ctx: Context<'_>) -> Result<serenity::GuildId, AppError> {
    ctx.guild_id()
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))
}

async fn load(ctx: Context<'_>, id: i64) -> Result<store::Menu, AppError> {
    let guild_id = guild_of(ctx)?.get() as i64;

    store::find(&ctx.data().db, guild_id, id)
        .await?
        .ok_or_else(|| {
            AppError::Message(format!("There's no menu #{id} here. Try `/rolemenu list`."))
        })
}

async fn republish(ctx: Context<'_>, record: &store::Menu) -> Result<(), AppError> {
    let choices = store::choices(&ctx.data().db, record.id).await?;
    let channel = serenity::ChannelId::new(record.channel_id as u64);

    let Some(message_id) = record.message_id else {
        return Ok(());
    };

    let edit = serenity::EditMessage::new()
        .embed(menu::embed(record, &choices))
        .components(menu::components(record, &choices));

    if let Err(e) = channel
        .edit_message(
            &ctx.http(),
            serenity::MessageId::new(message_id as u64),
            edit,
        )
        .await
    {
        tracing::warn!(?e, menu = record.id, "couldn't update the posted menu");
        return Err(AppError::Message(
            "Saved, but I couldn't update the posted message — check I can still see it.".into(),
        ));
    }

    Ok(())
}

#[poise::command(slash_command)]
pub async fn create(
    ctx: Context<'_>,
    #[description = "Where to post the menu"] channel: serenity::GuildChannel,
    #[description = "Heading shown on the menu"] title: String,
    #[description = "Optional blurb under the heading"] description: Option<String>,
    #[description = "How many roles a member may hold from this menu"]
    #[min = 1]
    #[max = 25]
    max_choices: Option<i32>,
) -> Result<(), AppError> {
    let guild_id = guild_of(ctx)?;
    let title = title.trim().to_string();

    if title.is_empty() || title.chars().count() > store::MAX_LABEL {
        return Err(AppError::Message(format!(
            "The title must be between 1 and {} characters.",
            store::MAX_LABEL
        )));
    }

    let id = store::create(
        &ctx.data().db,
        guild_id.get() as i64,
        channel.id.get() as i64,
        &title,
        description
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty()),
        max_choices.unwrap_or(store::MAX_OPTIONS as i32) as i16,
    )
    .await?;

    let record = load(ctx, id).await?;
    let choices = Vec::new();

    let posted = channel
        .id
        .send_message(
            &ctx.http(),
            serenity::CreateMessage::new()
                .embed(menu::embed(&record, &choices))
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        )
        .await
        .map_err(|e| {
            AppError::Message(format!(
                "Created the menu but couldn't post it in {channel} — {e}"
            ))
        })?;

    store::set_message(&ctx.data().db, id, posted.id.get() as i64).await?;

    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                "Menu **#{id}** posted in {channel}. Add roles with `/rolemenu add menu:{id}`."
            ))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn add(
    ctx: Context<'_>,
    #[description = "Which menu"] menu: i64,
    #[description = "Role members can pick"] role: serenity::Role,
    #[description = "Label shown in the dropdown (defaults to the role name)"] label: Option<
        String,
    >,
    #[description = "Short note under the label"] description: Option<String>,
) -> Result<(), AppError> {
    let record = load(ctx, menu).await?;

    if role.managed {
        return Err(AppError::Message(format!(
            "**{}** is managed by an integration, so nobody can be given it.",
            role.name
        )));
    }

    if role.id.get() == record.guild_id as u64 {
        return Err(AppError::Message(
            "That's the @everyone role — everyone already has it.".into(),
        ));
    }

    let label = label
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| role.name.clone());

    if label.chars().count() > store::MAX_LABEL {
        return Err(AppError::Message(format!(
            "The label must be {} characters or fewer.",
            store::MAX_LABEL
        )));
    }

    store::add_choice(
        &ctx.data().db,
        menu,
        role.id.get() as i64,
        &label,
        description
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty()),
    )
    .await?;

    republish(ctx, &record).await?;

    ctx.send(
        poise::CreateReply::default()
            .content(format!("Added **{label}** to menu #{menu}."))
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Which menu"] menu: i64,
    #[description = "Role to take off the menu"] role: serenity::Role,
) -> Result<(), AppError> {
    let record = load(ctx, menu).await?;

    if !store::remove_choice(&ctx.data().db, menu, role.id.get() as i64).await? {
        return Err(AppError::Message(format!(
            "**{}** isn't on menu #{menu}.",
            role.name
        )));
    }

    republish(ctx, &record).await?;

    ctx.send(
        poise::CreateReply::default()
            .content(format!("Removed **{}** from menu #{menu}.", role.name))
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), AppError> {
    let guild_id = guild_of(ctx)?.get() as i64;
    let menus = store::list(&ctx.data().db, guild_id).await?;

    if menus.is_empty() {
        return Err(AppError::Message(
            "No role menus here yet — make one with `/rolemenu create`.".into(),
        ));
    }

    let mut lines = Vec::new();

    for record in &menus {
        let count = store::choices(&ctx.data().db, record.id).await?.len();
        lines.push(format!(
            "**#{}** · {} — {count} role(s) in <#{}>",
            record.id, record.title, record.channel_id
        ));
    }

    ctx.send(
        poise::CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title("Role menus")
                    .description(lines.join("\n")),
            )
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Which menu to delete"] menu: i64,
) -> Result<(), AppError> {
    let guild_id = guild_of(ctx)?.get() as i64;

    let Some(record) = store::delete(&ctx.data().db, guild_id, menu).await? else {
        return Err(AppError::Message(format!("There's no menu #{menu} here.")));
    };

    if let Some(message_id) = record.message_id {
        let channel = serenity::ChannelId::new(record.channel_id as u64);

        if let Err(e) = channel
            .delete_message(&ctx.http(), serenity::MessageId::new(message_id as u64))
            .await
        {
            tracing::warn!(?e, menu, "couldn't delete the posted menu message");
        }
    }

    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                "Deleted menu #{menu}. Roles members already hold are untouched."
            ))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
