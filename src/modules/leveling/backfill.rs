use crate::error::AppError;
use sqlx::PgPool;

use super::{achievements, setup::xp, store};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub profiles: usize,
    pub coins: i64,
    pub achievements: usize,
    pub badges: usize,
    pub failures: usize,
}

pub async fn run(db: &PgPool) -> Result<Summary, AppError> {
    let pending = store::backfill_candidates(db).await?;
    if pending.is_empty() {
        return Ok(Summary::default());
    }

    tracing::info!(count = pending.len(), "backfilling profiles");
    let mut summary = Summary::default();

    for candidate in pending {
        let earned = achievements::activity_coins(candidate.experience);
        let level = xp::level_for_xp(candidate.best_guild_experience);

        let ids: Vec<String> = achievements::earned_at(level)
            .map(|a| a.id.to_string())
            .collect();

        let award = match store::backfill_user(db, candidate.user_id, earned, &ids).await {
            Ok(award) => award,
            Err(e) => {
                tracing::warn!(?e, user = candidate.user_id, "failed to backfill profile");
                summary.failures += 1;
                continue;
            }
        };

        summary.profiles += 1;
        summary.coins += earned + award.coins;
        summary.achievements += award.unlocked.len();
        summary.badges += award.badges.len();

        tracing::debug!(
            user = candidate.user_id,
            level,
            activity_coins = earned,
            achievement_coins = award.coins,
            "backfilled profile"
        );
    }

    tracing::info!(
        profiles = summary.profiles,
        coins = summary.coins,
        achievements = summary.achievements,
        badges = summary.badges,
        failures = summary.failures,
        "backfill complete"
    );

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_coins_track_whole_ticks() {
        let per_tick = xp::XP_PER_MESSAGE;

        assert_eq!(achievements::activity_coins(0), 0);
        assert_eq!(achievements::activity_coins(per_tick - 1), 0);
        assert_eq!(
            achievements::activity_coins(per_tick),
            achievements::COINS_PER_TICK
        );
        assert_eq!(
            achievements::activity_coins(per_tick * 10),
            achievements::COINS_PER_TICK * 10
        );
    }

    #[test]
    fn negative_experience_earns_nothing() {
        assert_eq!(achievements::activity_coins(-5_000), 0);
    }

    #[test]
    fn a_realistic_account_gets_a_sensible_balance() {
        let coins = achievements::activity_coins(4_900);
        assert_eq!(coins, 650);

        let payout: i64 = achievements::earned_at(7).map(|a| a.coins).sum();
        assert_eq!(payout, 350);
        assert_eq!(coins + payout, 1_000);
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires the dev database"]
    async fn backfill_is_idempotent() {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let db = PgPool::connect(&url)
            .await
            .expect("connect to the database");

        let first = run(&db).await.expect("first pass");
        println!("first pass:  {first:?}");

        let second = run(&db).await.expect("second pass");
        println!("second pass: {second:?}");

        assert_eq!(
            second,
            Summary::default(),
            "a second pass must not pay anything out again"
        );
    }
}
