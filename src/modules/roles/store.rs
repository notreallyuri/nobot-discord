use crate::error::AppError;
use sqlx::PgPool;

pub const MAX_OPTIONS: usize = 25;
pub const MAX_LABEL: usize = 100;

#[derive(Debug)]
pub struct Menu {
    pub id: i64,
    pub guild_id: i64,
    pub channel_id: i64,
    pub message_id: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub min_choices: i16,
    pub max_choices: i16,
}

#[derive(Debug, Clone)]
pub struct Choice {
    pub role_id: i64,
    pub label: String,
    pub description: Option<String>,
}

pub async fn create(
    db: &PgPool,
    guild_id: i64,
    channel_id: i64,
    title: &str,
    description: Option<&str>,
    max_choices: i16,
) -> Result<i64, AppError> {
    let id = sqlx::query_scalar!(
        "INSERT INTO role_menu (guild_id, channel_id, title, description, max_choices)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
        guild_id,
        channel_id,
        title,
        description,
        max_choices,
    )
    .fetch_one(db)
    .await?;

    Ok(id)
}

pub async fn find(db: &PgPool, guild_id: i64, id: i64) -> Result<Option<Menu>, AppError> {
    let menu = sqlx::query_as!(
        Menu,
        "SELECT id, guild_id, channel_id, message_id, title, description,
                min_choices, max_choices
         FROM role_menu WHERE guild_id = $1 AND id = $2",
        guild_id,
        id,
    )
    .fetch_optional(db)
    .await?;

    Ok(menu)
}

pub async fn by_id(db: &PgPool, id: i64) -> Result<Option<Menu>, AppError> {
    let menu = sqlx::query_as!(
        Menu,
        "SELECT id, guild_id, channel_id, message_id, title, description,
                min_choices, max_choices
         FROM role_menu WHERE id = $1",
        id,
    )
    .fetch_optional(db)
    .await?;

    Ok(menu)
}

pub async fn list(db: &PgPool, guild_id: i64) -> Result<Vec<Menu>, AppError> {
    let menus = sqlx::query_as!(
        Menu,
        "SELECT id, guild_id, channel_id, message_id, title, description,
                min_choices, max_choices
         FROM role_menu WHERE guild_id = $1 ORDER BY id",
        guild_id,
    )
    .fetch_all(db)
    .await?;

    Ok(menus)
}

pub async fn choices(db: &PgPool, menu_id: i64) -> Result<Vec<Choice>, AppError> {
    let rows = sqlx::query_as!(
        Choice,
        "SELECT role_id, label, description FROM role_menu_option
         WHERE menu_id = $1 ORDER BY position, role_id",
        menu_id,
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn add_choice(
    db: &PgPool,
    menu_id: i64,
    role_id: i64,
    label: &str,
    description: Option<&str>,
) -> Result<(), AppError> {
    let count = sqlx::query_scalar!(
        "SELECT count(*) FROM role_menu_option WHERE menu_id = $1",
        menu_id
    )
    .fetch_one(db)
    .await?
    .unwrap_or(0);

    let exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM role_menu_option WHERE menu_id = $1 AND role_id = $2)",
        menu_id,
        role_id,
    )
    .fetch_one(db)
    .await?
    .unwrap_or(false);

    if !exists && count as usize >= MAX_OPTIONS {
        return Err(AppError::Message(format!(
            "A menu can hold at most {MAX_OPTIONS} roles."
        )));
    }

    sqlx::query!(
        "INSERT INTO role_menu_option (menu_id, role_id, label, description, position)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (menu_id, role_id)
         DO UPDATE SET label = $3, description = $4",
        menu_id,
        role_id,
        label,
        description,
        count as i32,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn remove_choice(db: &PgPool, menu_id: i64, role_id: i64) -> Result<bool, AppError> {
    let result = sqlx::query!(
        "DELETE FROM role_menu_option WHERE menu_id = $1 AND role_id = $2",
        menu_id,
        role_id,
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn set_message(db: &PgPool, menu_id: i64, message_id: i64) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE role_menu SET message_id = $2, updated_at = now() WHERE id = $1",
        menu_id,
        message_id,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn delete(db: &PgPool, guild_id: i64, id: i64) -> Result<Option<Menu>, AppError> {
    let menu = sqlx::query_as!(
        Menu,
        "DELETE FROM role_menu WHERE guild_id = $1 AND id = $2
         RETURNING id, guild_id, channel_id, message_id, title, description,
                   min_choices, max_choices",
        guild_id,
        id,
    )
    .fetch_optional(db)
    .await?;

    Ok(menu)
}
