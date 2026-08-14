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
use std::sync::OnceLock;

pub const PREFIX: &str = "shop:";
const PAGE_SIZE: usize = 6;

/// One shelf per aisle, rendered once for the life of the process. The catalogue
/// is static, so these never change, and holding the image steady across a page
/// turn is what lets a press avoid touching attachments at all.
static SHELVES: OnceLock<Vec<(String, Vec<u8>)>> = OnceLock::new();

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

    fn tab(self) -> &'static str {
        match self {
            Aisle::Badges => "Badges",
            Aisle::Boosters => "Boosters",
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

/// Where a press wants to go. The whole position lives in the id, so a shop
/// posted before a restart still works afterwards.
pub fn parse(custom_id: &str) -> Option<(Aisle, usize)> {
    let rest = custom_id.strip_prefix(PREFIX)?.strip_prefix("page:")?;
    let (aisle, page) = rest.split_once(':')?;

    Some((Aisle::from_slug(aisle)?, page.parse().ok()?))
}

pub type Wallet = store::Purse;

pub async fn wallet(db: &sqlx::PgPool, user_id: i64) -> Result<Wallet, AppError> {
    store::purse(db, user_id).await
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

pub fn embed(
    aisle: Aisle,
    page: usize,
    wallet: &Wallet,
    currency: &str,
    image: &str,
) -> serenity::CreateEmbed {
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
        .image(image)
        .footer(serenity::CreateEmbedFooter::new(footer))
}

pub fn shelf_name(aisle: Aisle) -> String {
    format!("shelf-{}.png", aisle.slug())
}

pub fn attached(aisle: Aisle) -> String {
    format!("attachment://{}", shelf_name(aisle))
}

fn cells_for(aisle: Aisle) -> Vec<Cell<'static>> {
    match aisle {
        Aisle::Badges => badges::purchasable()
            .map(|badge| Cell {
                emblem: Emblem {
                    icon: badge.icon,
                    colour: badge.colour,
                },
                label: badge.name,
            })
            .collect(),
        Aisle::Boosters => boosters::catalogue()
            .map(|booster| Cell {
                emblem: Emblem {
                    icon: booster.icon,
                    colour: booster.colour,
                },
                label: booster.name,
            })
            .collect(),
    }
}

fn build_shelves() -> Vec<(String, Vec<u8>)> {
    let mut built = Vec::new();

    for aisle in Aisle::ALL {
        let cells = cells_for(aisle);
        let (width, height) = strip::size(cells.len());

        match card::render(&strip::svg(&cells), width, height, card::SUPERSAMPLE) {
            Ok(png) => built.push((shelf_name(aisle), png)),
            Err(e) => tracing::warn!(?e, "could not draw a shop shelf"),
        }
    }

    built
}

/// A Discord embed carries one image and no per-line thumbnails, so an aisle
/// shows all of its wares as a labelled grid. It stays the same across a page
/// turn on purpose: an unchanged image means the press sends no attachment,
/// which is the difference between a page turn feeling instant and waiting on
/// an upload.
async fn shelves() -> &'static [(String, Vec<u8>)] {
    if let Some(built) = SHELVES.get() {
        return built;
    }

    let built = tokio::task::spawn_blocking(build_shelves)
        .await
        .unwrap_or_default();

    SHELVES.get_or_init(|| built)
}

pub async fn shelf(aisle: Aisle) -> Option<serenity::CreateAttachment> {
    let name = shelf_name(aisle);

    shelves()
        .await
        .iter()
        .find(|(shelf, _)| *shelf == name)
        .map(|(shelf, png)| serenity::CreateAttachment::bytes(png.clone(), shelf))
}

/// Discord gives a select menu a whole action row and will not let buttons
/// share it, so the aisles are buttons too. Two aisles plus Back and Next is
/// four of the five a row allows; a third aisle would have to give that up.
pub fn components(aisle: Aisle, page: usize) -> Vec<serenity::CreateActionRow> {
    let last = pages(aisle) - 1;
    let page = page.min(last);

    let mut row: Vec<serenity::CreateButton> = Aisle::ALL
        .iter()
        .map(|entry| {
            serenity::CreateButton::new(page_id(*entry, 0))
                .label(entry.tab())
                .style(if *entry == aisle {
                    serenity::ButtonStyle::Primary
                } else {
                    serenity::ButtonStyle::Secondary
                })
        })
        .collect();

    if last > 0 {
        row.push(
            serenity::CreateButton::new(page_id(aisle, page.saturating_sub(1)))
                .label("Back")
                .style(serenity::ButtonStyle::Secondary)
                .disabled(page == 0),
        );
        row.push(
            serenity::CreateButton::new(page_id(aisle, (page + 1).min(last)))
                .label("Next")
                .style(serenity::ButtonStyle::Secondary)
                .disabled(page == last),
        );
    }

    vec![serenity::CreateActionRow::Buttons(row)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_id_round_trips() {
        for aisle in Aisle::ALL {
            for page in [0, 1, 7] {
                let id = page_id(aisle, page);
                let (back, at) = parse(&id).unwrap_or_else(|| panic!("{id} did not parse"));

                assert_eq!(back.slug(), aisle.slug());
                assert_eq!(at, page);
            }
        }
    }

    #[test]
    fn every_control_fits_one_row_within_discords_limit() {
        for aisle in Aisle::ALL {
            let rows = components(aisle, 0);
            assert_eq!(rows.len(), 1, "{} should need one row", aisle.label());

            let serenity::CreateActionRow::Buttons(buttons) = &rows[0] else {
                panic!("a select menu cannot share a row with buttons");
            };

            assert!(
                buttons.len() <= 5,
                "{} needs {} buttons, more than a row holds",
                aisle.label(),
                buttons.len()
            );
        }
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

        let embed = embed(Aisle::Badges, 99, &wallet, "coins", "attachment://x.png");
        let rendered = format!("{embed:?}");

        assert!(
            rendered.contains("Paragon"),
            "the last page should be shown"
        );
    }

    #[test]
    fn a_single_page_aisle_offers_no_turn_buttons() {
        let one_page = components(Aisle::Boosters, 0);
        let many_pages = components(Aisle::Badges, 0);

        let count = |rows: &[serenity::CreateActionRow]| match &rows[0] {
            serenity::CreateActionRow::Buttons(buttons) => buttons.len(),
            _ => panic!("expected buttons"),
        };

        assert_eq!(count(&one_page), Aisle::ALL.len());
        assert_eq!(count(&many_pages), Aisle::ALL.len() + 2);
    }
}
