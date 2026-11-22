//! The household, its members and the local device's link to its member.

use rusqlite::OptionalExtension;

use crate::{Result, Store, Visibility, Write};

/// The household row (schema v2: typed settings columns).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HouseholdRow {
    pub id: String,
    pub name: String,
    pub base_currency_code: String,
    pub budget_mode: String,
    pub locale: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRow {
    pub id: String,
    pub display_name: String,
    pub color: Option<String>,
}

impl Store {
    /// The single live household of this database, if onboarding created it.
    pub fn household(&self) -> Result<Option<HouseholdRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, name, base_currency_code, budget_mode, locale FROM household \
                 WHERE deleted = 0 ORDER BY id LIMIT 1",
                [],
                |r| {
                    Ok(HouseholdRow {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        base_currency_code: r.get(2)?,
                        budget_mode: r.get(3)?,
                        locale: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// Live members, by id.
    pub fn members(&self) -> Result<Vec<MemberRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, display_name, color FROM member WHERE deleted = 0 ORDER BY id")?;
        let rows = stmt
            .query_map([], |r| {
                Ok(MemberRow {
                    id: r.get(0)?,
                    display_name: r.get(1)?,
                    color: r.get(2)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// The member the local device belongs to; `None` before onboarding.
    pub fn local_member_id(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT member_id FROM device WHERE is_local = 1", [], |r| {
                r.get(0)
            })
            .optional()?
            .flatten())
    }
}

impl Write<'_> {
    pub fn insert_household(&self, h: &HouseholdRow) -> Result<()> {
        self.insert(
            "household",
            &h.id,
            Visibility::Household,
            &[
                ("id", &h.id),
                ("name", &h.name),
                ("base_currency_code", &h.base_currency_code),
                ("budget_mode", &h.budget_mode),
                ("locale", &h.locale),
            ],
        )
    }

    pub fn insert_member(&self, household_id: &str, m: &MemberRow) -> Result<()> {
        self.insert(
            "member",
            &m.id,
            Visibility::Household,
            &[
                ("id", &m.id),
                ("household_id", &household_id),
                ("display_name", &m.display_name),
                ("color", &m.color),
            ],
        )
    }

    /// Links the local device to its member. `device` is a local-only trust store, so this
    /// write is not logged (ADR-0010).
    pub fn link_local_device(&self, member_id: &str) -> Result<()> {
        self.tx().execute(
            "UPDATE device SET member_id = ?1 WHERE is_local = 1",
            [member_id],
        )?;
        Ok(())
    }
}
