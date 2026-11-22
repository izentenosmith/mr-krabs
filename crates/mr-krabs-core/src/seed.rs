//! The default household data: the single source of the seed tree in docs/01.
//!
//! Seeded rows are ordinary user data afterwards: they can be renamed, moved, merged or
//! deleted. Their ids are `UUIDv5(MRK_SEED_NAMESPACE, seed_key)`, so every device seeds
//! identical rows and pairing never duplicates them (ADR-0008). Currencies are seeded by
//! migration 0002 (natural primary key).

use mrk_domain::normalize_name;
use mrk_store::{SeedAccount, SeedCategory};
use uuid::Uuid;

/// Fixed namespace for seeded ids. Never change it: existing households would stop
/// recognizing their seeded rows and seed them again (ADR-0008).
pub const MRK_SEED_NAMESPACE: Uuid = Uuid::from_u128(0x5713fc3c_cb23_4e40_a7f0_ef2344a52494);

/// The language seed names are written in, from `household.locale`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    EsCl,
    En,
}

impl Locale {
    pub fn parse(tag: &str) -> Option<Self> {
        match tag {
            "es-CL" => Some(Self::EsCl),
            "en" => Some(Self::En),
            _ => None,
        }
    }
}

/// One node of the default category tree. A `seed_key` is never reused for other content.
struct CategorySeed {
    slug: &'static str,
    parent: Option<&'static str>,
    kind: &'static str,
    es_cl: &'static str,
    en: &'static str,
}

const fn root(
    slug: &'static str,
    kind: &'static str,
    es_cl: &'static str,
    en: &'static str,
) -> CategorySeed {
    CategorySeed {
        slug,
        parent: None,
        kind,
        es_cl,
        en,
    }
}

const fn child(
    parent: &'static str,
    slug: &'static str,
    kind: &'static str,
    es_cl: &'static str,
    en: &'static str,
) -> CategorySeed {
    CategorySeed {
        slug,
        parent: Some(parent),
        kind,
        es_cl,
        en,
    }
}

const I: &str = "income";
const E: &str = "expense";

/// docs/01 "Default category tree", parents before children.
const CATEGORIES: &[CategorySeed] = &[
    root("income", I, "Ingresos", "Income"),
    child("income", "income.salary", I, "Sueldo", "Salary"),
    child(
        "income",
        "income.other",
        I,
        "Otros ingresos",
        "Other income",
    ),
    root("expense.fixed", E, "Gastos fijos", "Fixed costs"),
    child(
        "expense.fixed",
        "expense.fixed.housing",
        E,
        "Arriendo / Dividendo",
        "Rent / Mortgage",
    ),
    child(
        "expense.fixed",
        "expense.fixed.common_charges",
        E,
        "Gastos comunes",
        "Building fees",
    ),
    child("expense.fixed", "expense.fixed.gas", E, "Gas", "Gas"),
    child(
        "expense.fixed",
        "expense.fixed.electricity",
        E,
        "Luz",
        "Electricity",
    ),
    child("expense.fixed", "expense.fixed.water", E, "Agua", "Water"),
    child(
        "expense.fixed",
        "expense.fixed.internet",
        E,
        "Internet",
        "Internet",
    ),
    child(
        "expense.fixed",
        "expense.fixed.domestic_help",
        E,
        "Servicio doméstico",
        "Domestic help",
    ),
    root("expense.variable", E, "Gastos variables", "Variable costs"),
    child(
        "expense.variable",
        "expense.variable.household",
        E,
        "Compras casa",
        "Household shopping",
    ),
    child(
        "expense.variable",
        "expense.variable.repairs",
        E,
        "Arreglos casa",
        "Home repairs",
    ),
    root("expense.food", E, "Comida", "Food"),
    child(
        "expense.food",
        "expense.food.supermarket",
        E,
        "Supermercado",
        "Groceries",
    ),
    child(
        "expense.food",
        "expense.food.delivery",
        E,
        "Delivery",
        "Delivery",
    ),
    child(
        "expense.food",
        "expense.food.restaurants",
        E,
        "Restaurantes",
        "Restaurants",
    ),
    root("expense.transport", E, "Transporte", "Transport"),
    child(
        "expense.transport",
        "expense.transport.fuel",
        E,
        "Bencina",
        "Fuel",
    ),
    child(
        "expense.transport",
        "expense.transport.car_maintenance",
        E,
        "Mantención auto",
        "Car maintenance",
    ),
    child(
        "expense.transport",
        "expense.transport.public",
        E,
        "Transporte público",
        "Public transport",
    ),
    child(
        "expense.transport",
        "expense.transport.parking_tolls",
        E,
        "Estacionamiento/TAG",
        "Parking & tolls",
    ),
    root("expense.subscriptions", E, "Suscripciones", "Subscriptions"),
    child(
        "expense.subscriptions",
        "expense.subscriptions.streaming",
        E,
        "Streaming",
        "Streaming",
    ),
    child(
        "expense.subscriptions",
        "expense.subscriptions.software",
        E,
        "Software",
        "Software",
    ),
    child(
        "expense.subscriptions",
        "expense.subscriptions.other",
        E,
        "Otras",
        "Other",
    ),
    root("expense.leisure", E, "Ocio", "Leisure"),
    child(
        "expense.leisure",
        "expense.leisure.travel",
        E,
        "Viajes",
        "Travel",
    ),
    child(
        "expense.leisure",
        "expense.leisure.cinema",
        E,
        "Cine",
        "Cinema",
    ),
    child(
        "expense.leisure",
        "expense.leisure.going_out",
        E,
        "Salidas",
        "Going out",
    ),
    root("expense.debt", E, "Deudas", "Debt"),
    child(
        "expense.debt",
        "expense.debt.interest_fees",
        E,
        "Intereses y comisiones",
        "Interest & fees",
    ),
];

/// The spreadsheet's "Inversiones y ahorros" rows: accounts, not categories (docs/01).
struct AccountSeed {
    slug: &'static str,
    kind: &'static str,
    es_cl: &'static str,
    en: &'static str,
}

const ACCOUNTS: &[AccountSeed] = &[
    AccountSeed {
        slug: "account.emergency_fund",
        kind: "savings",
        es_cl: "Fondo de emergencia",
        en: "Emergency fund",
    },
    AccountSeed {
        slug: "account.savings",
        kind: "savings",
        es_cl: "Ahorro",
        en: "Savings",
    },
    AccountSeed {
        slug: "account.investments",
        kind: "investment",
        es_cl: "Inversiones",
        en: "Investments",
    },
];

/// Deterministic id of a seeded row.
pub fn seed_id(seed_key: &str) -> String {
    Uuid::new_v5(&MRK_SEED_NAMESPACE, seed_key.as_bytes()).to_string()
}

/// The default categories in the household's language, parents first.
pub fn seed_categories(locale: Locale) -> Vec<SeedCategory> {
    CATEGORIES
        .iter()
        .zip(0i64..)
        .map(|(c, order)| {
            let name = match locale {
                Locale::EsCl => c.es_cl,
                Locale::En => c.en,
            };
            SeedCategory {
                id: seed_id(c.slug),
                parent_id: c.parent.map(seed_id),
                kind: c.kind,
                name: name.to_owned(),
                name_key: normalize_name(name),
                seed_key: c.slug.to_owned(),
                sort_order: order,
            }
        })
        .collect()
}

/// The default savings and investment accounts, in the household's base currency.
pub fn seed_accounts(locale: Locale, base_currency: &str) -> Vec<SeedAccount> {
    ACCOUNTS
        .iter()
        .zip(0i64..)
        .map(|(a, order)| SeedAccount {
            id: seed_id(a.slug),
            name: match locale {
                Locale::EsCl => a.es_cl,
                Locale::En => a.en,
            }
            .to_owned(),
            kind: a.kind,
            currency_code: base_currency.to_owned(),
            sort_order: order,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn seed_ids_are_deterministic_and_distinct() {
        assert_eq!(seed_id("expense.food"), seed_id("expense.food"));
        let keys: BTreeSet<_> = CATEGORIES
            .iter()
            .map(|c| c.slug)
            .chain(ACCOUNTS.iter().map(|a| a.slug))
            .collect();
        assert_eq!(
            keys.len(),
            CATEGORIES.len() + ACCOUNTS.len(),
            "a seed_key is used twice"
        );
        let ids: BTreeSet<_> = keys.iter().map(|k| seed_id(k)).collect();
        assert_eq!(ids.len(), keys.len());
        // Pinned so an accidental namespace change fails loudly (ADR-0008).
        assert_eq!(
            seed_id("expense.food"),
            Uuid::new_v5(&MRK_SEED_NAMESPACE, b"expense.food").to_string()
        );
        assert_eq!(
            Uuid::parse_str(&seed_id("income"))
                .unwrap()
                .get_version_num(),
            5
        );
    }

    #[test]
    fn parents_come_first_and_share_the_kind() {
        let mut seen = std::collections::BTreeMap::new();
        for c in CATEGORIES {
            if let Some(p) = c.parent {
                assert_eq!(
                    seen.get(p),
                    Some(&c.kind),
                    "{} before or across its parent",
                    c.slug
                );
            }
            seen.insert(c.slug, c.kind);
        }
    }

    #[test]
    fn names_are_localized_and_normalized() {
        let es = seed_categories(Locale::EsCl);
        let en = seed_categories(Locale::En);
        let doc = es
            .iter()
            .find(|c| c.seed_key == "expense.fixed.domestic_help")
            .unwrap();
        assert_eq!(doc.name, "Servicio doméstico");
        assert_eq!(doc.name_key, "servicio domestico");
        assert_eq!(
            en.iter()
                .find(|c| c.seed_key == "expense.food")
                .unwrap()
                .name,
            "Food"
        );
        // Ids depend only on the seed_key, never on the language.
        assert_eq!(
            es.iter().map(|c| &c.id).collect::<Vec<_>>(),
            en.iter().map(|c| &c.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn savings_are_accounts_not_categories() {
        assert!(
            !CATEGORIES
                .iter()
                .any(|c| c.es_cl == "Ahorro" || c.es_cl == "Inversiones")
        );
        let accounts = seed_accounts(Locale::EsCl, "CLP");
        assert_eq!(accounts.len(), 3);
        assert!(accounts.iter().all(|a| a.currency_code == "CLP"));
    }
}
