use super::{badges, boosters, store};
use crate::{
    card::{
        self,
        emblem::Emblem,
        strip::{self, Cell},
    },
    error::AppError,
};
use poise::serenity_prelude as serenity;

pub const PREFIX: &str = "shop:";
pub const IMAGE: &str = "shop.png";
const PAGE_SIZE: usize = 6;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Aisle {
    Badges,
    Boosters,
}

impl Aisle {
    fn slug(self) -> &'static str {
        match self {
            Aisle::Badges => "badges",
            Aisle::Boosters => "boosters",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Aisle::Badges => "Badges",
            Aisle::Boosters => "XP boosters",
        }
    }

    fn blurb(self) -> &'static str {
        match self {
            Aisle::Badges => "Worn on your profile card. Bought once.",
            Aisle::Boosters => "Multiply the XP you earn. Spent as they run.",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "badges" => Some(Aisle::Badges),
            "boosters" => Some(Aisle::Boosters),
            _ => None,
        }
    }

    fn count(self) -> usize {
        match self {
            Aisle::Badges => badges::purchasable().count(),
            Aisle::Boosters => boosters::catalogue().count(),
        }
    }

    pub const ALL: [Aisle; 2] = [Aisle::Badges, Aisle::Boosters];
}

pub fn page_id(aisle: Aisle, page: usize) -> String {
    format!("{PREFIX}page:{}:{page}", aisle.slug())
}

pub fn select_id() -> String {
    format!("{PREFIX}aisle")
}

/// Where a press wants to go. The whole position lives in the id, so a shop
/// posted before a restart still works afterwards.
pub enum Move {
    Turn(Aisle, usize),
    Switch,
}

pub fn parse(custom_id: &str) -> Option<Move> {
    let rest = custom_id.strip_prefix(PREFIX)?;

    if rest == "aisle" {
        return Some(Move::Switch);
    }

    let (aisle, page) = rest.strip_prefix("page:")?.split_once(':')?;
    Some(Move::Turn(Aisle::from_slug(aisle)?, page.parse().ok()?))
}

pub struct Wallet {
    pub balance: i64,
    pub owned: Vec<String>,
    pub boost: Option<store::ActiveBooster>,
}

pub async fn wallet(db: &sqlx::PgPool, user_id: i64) -> Result<Wallet, AppError> {
    Ok(Wallet {
        balance: store::balance(db, user_id).await?,
        owned: store::owned_badges(db, user_id)
            .await?
            .into_iter()
            .map(|badge| badge.badge_id)
            .collect(),
        boost: store::active_booster(db, user_id).await?,
    })
}

fn pages(aisle: Aisle) -> usize {
    aisle.count().div_ceil(PAGE_SIZE).max(1)
}

fn afford(price: i64, balance: i64, currency: &str) -> String {
    if balance >= price {
        format!("{price} {currency}")
    } else {
        format!("{price} {currency} — {} short", price - balance)
    }
}

fn remaining(boost: &store::ActiveBooster) -> String {
    let Some(expires_at) = boost.expires_at else {
        return "permanently".to_string();
    };

    let left = expires_at - sqlx::types::chrono::Utc::now();
    let minutes = left.num_minutes().max(0);

    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m left"),
        (h, 0) => format!("{h}h left"),
        (h, m) => format!("{h}h {m}m left"),
    }
}

pub fn embed(aisle: Aisle, page: usize, wallet: &Wallet, currency: &str) -> serenity::CreateEmbed {
    let page = page.min(pages(aisle) - 1);
    let skip = page * PAGE_SIZE;

    let body: Vec<String> = match aisle {
        Aisle::Badges => badges::purchasable()
            .skip(skip)
            .take(PAGE_SIZE)
            .map(|badge| {
                let price = badge.price.expect("purchasable");
                let status = if wallet.owned.iter().any(|id| id == badge.id) {
                    "owned".to_string()
                } else {
                    afford(price, wallet.balance, currency)
                };

                format!("**{}** — {status}\n{}", badge.name, badge.description)
            })
            .collect(),
        Aisle::Boosters => boosters::catalogue()
            .skip(skip)
            .take(PAGE_SIZE)
            .map(|booster| {
                format!(
                    "**{}** — {}\n{} · {}",
                    booster.name,
                    afford(booster.price, wallet.balance, currency),
                    booster.label(),
                    booster.description
                )
            })
            .collect(),
    };

    let mut footer = format!("{} {currency}", wallet.balance);
    if let Some(boost) = &wallet.boost {
        footer.push_str(&format!(
            " · {}x XP, {}",
            boost.multiplier_pct as f64 / boosters::NORMAL_PCT as f64,
            remaining(boost)
        ));
    }
    if pages(aisle) > 1 {
        footer.push_str(&format!(" · page {} of {}", page + 1, pages(aisle)));
    }

    serenity::CreateEmbed::new()
        .title(aisle.label())
        .description(format!("{}\n\n{}", aisle.blurb(), body.join("\n\n")))
        .image(format!("attachment://{IMAGE}"))
        .footer(serenity::CreateEmbedFooter::new(footer))
}

/// A Discord embed carries one image, not one per line, so the page's items are
/// drawn as a single labelled row in the same order the text lists them.
pub async fn shelf(aisle: Aisle, page: usize) -> Result<Vec<u8>, AppError> {
    let page = page.min(pages(aisle) - 1);
    let skip = page * PAGE_SIZE;

    let cells: Vec<Cell<'static>> = match aisle {
        Aisle::Badges => badges::purchasable()
            .skip(skip)
            .take(PAGE_SIZE)
            .map(|badge| Cell {
                emblem: Emblem {
                    icon: badge.icon,
                    colour: badge.colour,
                },
                label: badge.name,
            })
            .collect(),
        Aisle::Boosters => boosters::catalogue()
            .skip(skip)
            .take(PAGE_SIZE)
            .map(|booster| Cell {
                emblem: Emblem {
                    icon: booster.icon,
                    colour: booster.colour,
                },
                label: booster.name,
            })
            .collect(),
    };

    let (width, height) = strip::size(cells.len());
    card::render_async(strip::svg(&cells), width, height, card::SUPERSAMPLE).await
}

pub fn components(aisle: Aisle, page: usize) -> Vec<serenity::CreateActionRow> {
    let last = pages(aisle) - 1;
    let page = page.min(last);

    let options: Vec<serenity::CreateSelectMenuOption> = Aisle::ALL
        .iter()
        .map(|entry| {
            serenity::CreateSelectMenuOption::new(entry.label(), entry.slug())
                .description(entry.blurb())
                .default_selection(*entry == aisle)
        })
        .collect();

    let picker = serenity::CreateActionRow::SelectMenu(serenity::CreateSelectMenu::new(
        select_id(),
        serenity::CreateSelectMenuKind::String { options },
    ));

    if last == 0 {
        return vec![picker];
    }

    let turn = serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(page_id(aisle, page.saturating_sub(1)))
            .label("Back")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(page == 0),
        serenity::CreateButton::new(page_id(aisle, (page + 1).min(last)))
            .label("More")
            .style(serenity::ButtonStyle::Secondary)
            .disabled(page == last),
    ]);

    vec![picker, turn]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_id_round_trips() {
        for aisle in Aisle::ALL {
            for page in [0, 1, 7] {
                let id = page_id(aisle, page);
                match parse(&id) {
                    Some(Move::Turn(back, at)) => {
                        assert_eq!(back.slug(), aisle.slug());
                        assert_eq!(at, page);
                    }
                    _ => panic!("{id} did not parse back to a turn"),
                }
            }
        }
    }

    #[test]
    fn the_picker_id_parses_as_a_switch() {
        assert!(matches!(parse(&select_id()), Some(Move::Switch)));
    }

    #[test]
    fn ids_from_other_components_are_ignored() {
        for id in [
            "rolemenu:12",
            "shop",
            "shop:",
            "shop:page:badges",
            "nonsense",
        ] {
            assert!(parse(id).is_none(), "{id} should not parse");
        }
    }

    #[test]
    fn an_unknown_aisle_does_not_parse() {
        assert!(parse("shop:page:hats:0").is_none());
    }

    #[test]
    fn every_item_lands_on_some_page() {
        for aisle in Aisle::ALL {
            let seen: usize = (0..pages(aisle))
                .map(|page| {
                    let skip = page * PAGE_SIZE;
                    match aisle {
                        Aisle::Badges => badges::purchasable().skip(skip).take(PAGE_SIZE).count(),
                        Aisle::Boosters => boosters::catalogue().skip(skip).take(PAGE_SIZE).count(),
                    }
                })
                .sum();

            assert_eq!(seen, aisle.count(), "{} loses items", aisle.label());
        }
    }

    #[test]
    fn a_page_past_the_end_clamps_rather_than_emptying() {
        let wallet = Wallet {
            balance: 0,
            owned: Vec::new(),
            boost: None,
        };

        let embed = embed(Aisle::Badges, 99, &wallet, "coins");
        let rendered = format!("{embed:?}");

        assert!(
            rendered.contains("Paragon"),
            "the last page should be shown"
        );
    }

    #[test]
    fn a_single_page_aisle_offers_no_turn_buttons() {
        assert_eq!(components(Aisle::Boosters, 0).len(), 1);
        assert_eq!(components(Aisle::Badges, 0).len(), 2);
    }
}
