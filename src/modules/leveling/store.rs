use super::achievements;
use crate::{data::MemberId, error::AppError};
use sqlx::{PgConnection, PgPool};

#[derive(Debug)]
pub struct UserXp {
    pub guild_id: i64,
    pub user_id: i64,
    pub experience: i64,
}

#[derive(Debug)]
pub struct RankInfo {
    pub experience: i64,
    pub rank: i64,
}

#[derive(Debug)]
pub struct XpAward {
    pub experience: i64,
    pub granted: i64,
    pub multiplier_pct: i64,
}

pub async fn add_xp(db: &PgPool, key: MemberId, amount: i64) -> Result<XpAward, AppError> {
    let row = sqlx::query!(
        r#"WITH boost AS (
               SELECT COALESCE(MAX(multiplier_pct), 100)::bigint AS pct
                 FROM user_booster
                WHERE user_id = $2 AND (expires_at IS NULL OR expires_at > now())
           ),
           award AS (
               SELECT GREATEST($3 * pct / 100, 1) AS xp, pct FROM boost
           ),
           guild AS (
               INSERT INTO guild_member (guild_id, user_id, experience)
               SELECT $1, $2, xp FROM award
               ON CONFLICT (guild_id, user_id)
               DO UPDATE SET experience = guild_member.experience + EXCLUDED.experience
               RETURNING experience
           ),
           global AS (
               INSERT INTO users (user_id, experience)
               SELECT $2, xp FROM award
               ON CONFLICT (user_id)
               DO UPDATE SET experience = users.experience + EXCLUDED.experience
               RETURNING 1
           ),
           earned AS (
               INSERT INTO profile (user_id, coins) VALUES ($2, $4)
               ON CONFLICT (user_id)
               DO UPDATE SET coins = profile.coins + $4, updated_at = now()
               RETURNING 1
           )
           SELECT (SELECT experience FROM guild) AS "experience!",
                  (SELECT xp FROM award) AS "granted!",
                  (SELECT pct FROM award) AS "multiplier_pct!""#,
        key.guild_id,
        key.user_id,
        amount,
        achievements::COINS_PER_TICK,
    )
    .fetch_one(db)
    .await?;

    Ok(XpAward {
        experience: row.experience,
        granted: row.granted,
        multiplier_pct: row.multiplier_pct,
    })
}

#[derive(Debug)]
pub struct ActiveBooster {
    pub multiplier_pct: i64,
    pub expires_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

#[derive(Debug)]
pub struct Purse {
    pub balance: i64,
    pub owned: Vec<String>,
    pub boost: Option<ActiveBooster>,
}

pub async fn purse(db: &PgPool, user_id: i64) -> Result<Purse, AppError> {
    let row = sqlx::query!(
        r#"WITH live AS (
               SELECT multiplier_pct, expires_at
                 FROM user_booster
                WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > now())
                ORDER BY multiplier_pct DESC, expires_at DESC NULLS FIRST
                LIMIT 1
           )
           SELECT
               COALESCE((SELECT coins FROM profile WHERE user_id = $1), 0) AS "balance!",
               ARRAY(SELECT badge_id FROM user_badge WHERE user_id = $1) AS "owned!",
               (SELECT multiplier_pct::bigint FROM live) AS boost_pct,
               (SELECT expires_at FROM live) AS boost_until"#,
        user_id,
    )
    .fetch_one(db)
    .await?;

    Ok(Purse {
        balance: row.balance,
        owned: row.owned,
        boost: row.boost_pct.map(|multiplier_pct| ActiveBooster {
            multiplier_pct,
            expires_at: row.boost_until,
        }),
    })
}

pub async fn buy_booster(
    db: &PgPool,
    user_id: i64,
    multiplier_pct: i64,
    seconds: i64,
    price: i64,
) -> Result<Purchase, AppError> {
    let mut tx = db.begin().await?;

    let coins = sqlx::query_scalar!(
        "SELECT coins FROM profile WHERE user_id = $1 FOR UPDATE",
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0);

    if coins < price {
        return Ok(Purchase::TooPoor {
            balance: coins,
            price,
        });
    }

    let balance = sqlx::query_scalar!(
        "UPDATE profile SET coins = coins - $2, updated_at = now()
         WHERE user_id = $1 RETURNING coins",
        user_id,
        price,
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO user_booster (user_id, multiplier_pct, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))",
        user_id,
        multiplier_pct as i32,
        seconds as f64,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Purchase::Bought { balance })
}

pub async fn guild_rank(db: &PgPool, key: MemberId) -> Result<RankInfo, AppError> {
    let info = sqlx::query_as!(
        RankInfo,
        r#"WITH me AS (
               SELECT COALESCE(
                   (SELECT experience FROM guild_member WHERE guild_id = $1 AND user_id = $2),
                   0
               ) AS xp
           )
           SELECT
               me.xp AS "experience!",
               (SELECT COUNT(*) + 1 FROM guild_member
                 WHERE guild_id = $1 AND experience > me.xp) AS "rank!"
           FROM me"#,
        key.guild_id,
        key.user_id,
    )
    .fetch_one(db)
    .await?;

    Ok(info)
}

pub async fn leaderboard(db: &PgPool, guild_id: i64, limit: i64) -> Result<Vec<UserXp>, AppError> {
    let rows = sqlx::query_as!(
        UserXp,
        "SELECT guild_id, user_id, experience
         FROM guild_member WHERE guild_id = $1
         ORDER BY experience DESC LIMIT $2",
        guild_id,
        limit,
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

#[derive(Debug)]
pub struct ProfilePage {
    pub guild: RankInfo,
    pub global: RankInfo,
    pub coins: i64,
    pub accent: Option<i32>,
    pub background: Option<Vec<u8>>,
    pub background_blur: Option<Vec<u8>>,
    pub badges: Vec<String>,
}

pub async fn profile_page(db: &PgPool, key: MemberId) -> Result<ProfilePage, AppError> {
    let row = sqlx::query!(
        r#"WITH guild AS (
               SELECT COALESCE(
                   (SELECT experience FROM guild_member WHERE guild_id = $1 AND user_id = $2),
                   0
               ) AS xp
           ),
           global AS (
               SELECT COALESCE((SELECT experience FROM users WHERE user_id = $2), 0) AS xp
           )
           SELECT
               guild.xp AS "guild_experience!",
               (SELECT COUNT(*) + 1 FROM guild_member
                 WHERE guild_id = $1 AND experience > guild.xp) AS "guild_rank!",
               global.xp AS "global_experience!",
               (SELECT COUNT(*) + 1 FROM users WHERE experience > global.xp) AS "global_rank!",
               p.coins,
               p.accent,
               p.background,
               p.background_blur,
               ARRAY(
                   SELECT badge_id FROM user_badge
                    WHERE user_id = $2 AND equipped ORDER BY acquired_at
               ) AS "badges!"
           FROM guild CROSS JOIN global
           LEFT JOIN profile p ON p.user_id = $2"#,
        key.guild_id,
        key.user_id,
    )
    .fetch_one(db)
    .await?;

    Ok(ProfilePage {
        guild: RankInfo {
            experience: row.guild_experience,
            rank: row.guild_rank,
        },
        global: RankInfo {
            experience: row.global_experience,
            rank: row.global_rank,
        },
        coins: row.coins.unwrap_or(0),
        accent: row.accent,
        background: row.background,
        background_blur: row.background_blur,
        badges: row.badges,
    })
}

pub async fn accent(db: &PgPool, user_id: i64) -> Result<Option<i32>, AppError> {
    let accent = sqlx::query_scalar!("SELECT accent FROM profile WHERE user_id = $1", user_id)
        .fetch_optional(db)
        .await?;

    Ok(accent.flatten())
}

pub async fn set_background(
    db: &PgPool,
    user_id: i64,
    image: &[u8],
    blurred: &[u8],
) -> Result<(), AppError> {
    sqlx::query!(
        "INSERT INTO profile (user_id, background, background_blur)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id)
         DO UPDATE SET background = $2, background_blur = $3, updated_at = now()",
        user_id,
        image,
        blurred,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn set_accent(db: &PgPool, user_id: i64, accent: i32) -> Result<(), AppError> {
    sqlx::query!(
        "INSERT INTO profile (user_id, accent)
         VALUES ($1, $2)
         ON CONFLICT (user_id)
         DO UPDATE SET accent = $2, updated_at = now()",
        user_id,
        accent,
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn clear_background(db: &PgPool, user_id: i64) -> Result<bool, AppError> {
    let result = sqlx::query!(
        "UPDATE profile SET background = NULL, background_blur = NULL, updated_at = now()
         WHERE user_id = $1 AND background IS NOT NULL",
        user_id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn clear_accent(db: &PgPool, user_id: i64) -> Result<bool, AppError> {
    let result = sqlx::query!(
        "UPDATE profile SET accent = NULL, updated_at = now()
         WHERE user_id = $1 AND accent IS NOT NULL",
        user_id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

#[derive(Debug)]
pub struct OwnedBadge {
    pub badge_id: String,
    pub equipped: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Purchase {
    Bought { balance: i64 },
    AlreadyOwned,
    TooPoor { balance: i64, price: i64 },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Equip {
    Changed,
    NotOwned,
    NoChange,
    TooMany { limit: usize },
}

pub async fn owned_badges(db: &PgPool, user_id: i64) -> Result<Vec<OwnedBadge>, AppError> {
    let rows = sqlx::query_as!(
        OwnedBadge,
        "SELECT badge_id, equipped FROM user_badge
         WHERE user_id = $1 ORDER BY acquired_at",
        user_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn buy_badge(
    db: &PgPool,
    user_id: i64,
    badge_id: &str,
    price: i64,
) -> Result<Purchase, AppError> {
    let mut tx = db.begin().await?;

    let coins = sqlx::query_scalar!(
        "SELECT coins FROM profile WHERE user_id = $1 FOR UPDATE",
        user_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0);

    let owned = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM user_badge WHERE user_id = $1 AND badge_id = $2)",
        user_id,
        badge_id,
    )
    .fetch_one(&mut *tx)
    .await?
    .unwrap_or(false);

    if owned {
        return Ok(Purchase::AlreadyOwned);
    }

    if coins < price {
        return Ok(Purchase::TooPoor {
            balance: coins,
            price,
        });
    }

    let balance = sqlx::query_scalar!(
        "UPDATE profile SET coins = coins - $2, updated_at = now()
         WHERE user_id = $1 RETURNING coins",
        user_id,
        price,
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO user_badge (user_id, badge_id) VALUES ($1, $2)",
        user_id,
        badge_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Purchase::Bought { balance })
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Award {
    pub unlocked: Vec<String>,
    pub coins: i64,
    pub badges: Vec<&'static str>,
}

impl Award {
    pub fn achievements(&self) -> impl Iterator<Item = &'static achievements::Achievement> + '_ {
        self.unlocked.iter().filter_map(|id| achievements::find(id))
    }
}

async fn grant_awards(
    conn: &mut PgConnection,
    user_id: i64,
    candidates: &[String],
) -> Result<Award, AppError> {
    if candidates.is_empty() {
        return Ok(Award::default());
    }

    let unlocked = sqlx::query_scalar!(
        "INSERT INTO user_achievement (user_id, achievement_id)
         SELECT $1, unnest($2::text[])
         ON CONFLICT (user_id, achievement_id) DO NOTHING
         RETURNING achievement_id",
        user_id,
        candidates,
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut award = Award {
        unlocked,
        ..Default::default()
    };

    for achievement in award.achievements().collect::<Vec<_>>() {
        award.coins += achievement.coins;
        if let Some(badge) = achievement.badge {
            award.badges.push(badge);
        }
    }

    for badge in &award.badges {
        sqlx::query!(
            "INSERT INTO user_badge (user_id, badge_id) VALUES ($1, $2)
             ON CONFLICT (user_id, badge_id) DO NOTHING",
            user_id,
            badge,
        )
        .execute(&mut *conn)
        .await?;
    }

    Ok(award)
}

pub async fn award_achievements(
    db: &PgPool,
    user_id: i64,
    candidates: &[String],
) -> Result<Award, AppError> {
    let mut tx = db.begin().await?;
    let award = grant_awards(&mut tx, user_id, candidates).await?;

    if award.coins > 0 {
        sqlx::query!(
            "INSERT INTO profile (user_id, coins) VALUES ($1, $2)
             ON CONFLICT (user_id) DO UPDATE SET coins = profile.coins + $2, updated_at = now()",
            user_id,
            award.coins,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(award)
}

pub async fn set_equipped(
    db: &PgPool,
    user_id: i64,
    badge_id: &str,
    equipped: bool,
    limit: usize,
) -> Result<Equip, AppError> {
    let mut tx = db.begin().await?;

    let current = sqlx::query_scalar!(
        "SELECT equipped FROM user_badge WHERE user_id = $1 AND badge_id = $2 FOR UPDATE",
        user_id,
        badge_id,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(current) = current else {
        return Ok(Equip::NotOwned);
    };

    if current == equipped {
        return Ok(Equip::NoChange);
    }

    if equipped {
        let count = sqlx::query_scalar!(
            "SELECT count(*) FROM user_badge WHERE user_id = $1 AND equipped",
            user_id
        )
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(0);

        if count as usize >= limit {
            return Ok(Equip::TooMany { limit });
        }
    }

    sqlx::query!(
        "UPDATE user_badge SET equipped = $3 WHERE user_id = $1 AND badge_id = $2",
        user_id,
        badge_id,
        equipped,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Equip::Changed)
}

pub async fn unlocked_achievements(db: &PgPool, user_id: i64) -> Result<Vec<String>, AppError> {
    let rows = sqlx::query_scalar!(
        "SELECT achievement_id FROM user_achievement WHERE user_id = $1",
        user_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

#[derive(Debug)]
pub struct BackfillCandidate {
    pub user_id: i64,
    pub experience: i64,
    pub best_guild_experience: i64,
}

pub async fn backfill_candidates(db: &PgPool) -> Result<Vec<BackfillCandidate>, AppError> {
    let rows = sqlx::query_as!(
        BackfillCandidate,
        r#"SELECT
               u.user_id                          AS "user_id!",
               u.experience                       AS "experience!",
               COALESCE((
                   SELECT max(g.experience) FROM guild_member g WHERE g.user_id = u.user_id
               ), 0)                              AS "best_guild_experience!"
           FROM users u
           LEFT JOIN profile p ON p.user_id = u.user_id
           WHERE p.user_id IS NULL OR p.backfilled_at IS NULL
           ORDER BY u.user_id"#
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn backfill_user(
    db: &PgPool,
    user_id: i64,
    activity_coins: i64,
    candidates: &[String],
) -> Result<Award, AppError> {
    let mut tx = db.begin().await?;
    let award = grant_awards(&mut tx, user_id, candidates).await?;

    sqlx::query!(
        "INSERT INTO profile (user_id, coins, backfilled_at)
         VALUES ($1, $2::bigint + $3::bigint, now())
         ON CONFLICT (user_id) DO UPDATE
         SET coins = GREATEST(profile.coins, $2::bigint) + $3::bigint,
             backfilled_at = now(),
             updated_at = now()",
        user_id,
        activity_coins,
        award.coins,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(award)
}
