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

pub const SELECT_ID: &str = "shop:aisle";

/// Where a press wants to go. The whole position lives in the id, so a shop
/// posted before a restart still works afterwards.
/// Where a press wants to go. A pick carries its aisle in the chosen value
/// rather than the id, so the picker keeps one id however many aisles there are.
pub enum Move {
    Page(Aisle, usize),
    Pick,
}

pub fn parse(custom_id: &str) -> Option<Move> {
    if custom_id == SELECT_ID {
        return Some(Move::Pick);
    }

    let rest = custom_id.strip_prefix(PREFIX)?.strip_prefix("page:")?;
    let (aisle, page) = rest.split_once(':')?;

    Some(Move::Page(Aisle::from_slug(aisle)?, page.parse().ok()?))
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

/// Discord gives a select menu its own action row and will not let buttons
/// share it, so the picker and the page turns sit on separate rows. Aisles live
/// in the menu rather than in buttons because a row holds five, and buttons
/// would cap the shop at three categories.
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
        SELECT_ID,
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
            .label("Next")
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
                    Some(Move::Page(back, at)) => {
                        assert_eq!(back.slug(), aisle.slug());
                        assert_eq!(at, page);
                    }
                    _ => panic!("{id} did not parse back to a page"),
                }
            }
        }
    }

    fn custom_ids(rows: &[serenity::CreateActionRow]) -> Vec<String> {
        fn walk(value: &serde_json::Value, found: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, nested) in map {
                        if key == "custom_id"
                            && let Some(id) = nested.as_str()
                        {
                            found.push(id.to_string());
                        }
                        walk(nested, found);
                    }
                }
                serde_json::Value::Array(items) => items.iter().for_each(|it| walk(it, found)),
                _ => {}
            }
        }

        let payload = serde_json::to_value(rows).expect("components serialise");
        let mut found = Vec::new();
        walk(&payload, &mut found);
        found
    }

    #[test]
    fn no_page_repeats_a_custom_id() {
        for aisle in Aisle::ALL {
            for page in 0..pages(aisle) {
                let ids = custom_ids(&components(aisle, page));
                let mut unique = ids.clone();
                unique.sort();
                unique.dedup();

                assert_eq!(
                    unique.len(),
                    ids.len(),
                    "{} page {page} repeats a custom id: {ids:?}",
                    aisle.label()
                );
            }
        }
    }

    #[test]
    fn every_control_leads_somewhere_this_shop_understands() {
        for aisle in Aisle::ALL {
            for page in 0..pages(aisle) {
                for id in custom_ids(&components(aisle, page)) {
                    assert!(parse(&id).is_some(), "{id} does not parse back");
                }
            }
        }
    }

    #[test]
    fn the_picker_id_parses_as_a_pick() {
        assert!(matches!(parse(SELECT_ID), Some(Move::Pick)));
    }

    #[test]
    fn no_row_exceeds_what_discord_holds() {
        for aisle in Aisle::ALL {
            for row in components(aisle, 0) {
                if let serenity::CreateActionRow::Buttons(buttons) = row {
                    assert!(
                        buttons.len() <= 5,
                        "{} needs {} buttons, more than a row holds",
                        aisle.label(),
                        buttons.len()
                    );
                }
            }
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
        assert_eq!(components(Aisle::Boosters, 0).len(), 1);
        assert_eq!(components(Aisle::Badges, 0).len(), 2);
    }
}
