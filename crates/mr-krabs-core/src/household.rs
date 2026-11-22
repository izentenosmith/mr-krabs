//! Onboarding and the app's start-up state (REQ-1): `household_create`, `app_bootstrap`,
//! `settings_get`.

use mrk_store::{HouseholdRow, MemberRow};

use crate::api::{
    BootstrapDto, BudgetMode, HouseholdDto, HouseholdInput, Locale, MemberDto, SettingsDto,
};
use crate::support::{Fields, command, from_wire, new_id, wire};
use crate::{ApiError, App, ErrorCode};

/// Longest household, member or account name.
pub(crate) const NAME_MAX: usize = 100;

impl App {
    /// The household, its members, the member this device belongs to, the settings and the
    /// schema version. Works before onboarding (everything but the version is empty).
    pub fn bootstrap(&self) -> Result<BootstrapDto, ApiError> {
        command("app_bootstrap", || {
            let household = self.store.household()?;
            let members: Vec<MemberDto> =
                self.store.members()?.into_iter().map(member_dto).collect();
            let me = self
                .store
                .local_member_id()?
                .and_then(|id| members.iter().find(|m| m.id == id).cloned());
            Ok(BootstrapDto {
                settings: household.as_ref().map(settings_dto).transpose()?,
                household: household.as_ref().map(household_dto).transpose()?,
                members,
                me,
                schema_version: self.store.schema_version()?,
            })
        })
    }

    /// Onboarding: in one transaction, the household, me as its first member and the local
    /// device's link to me (X-2); then the default categories and accounts.
    pub fn household_create(&mut self, input: HouseholdInput) -> Result<HouseholdDto, ApiError> {
        command("household_create", || {
            let mut f = Fields::default();
            let name = f.name("name", &input.name, NAME_MAX);
            let my_name = f.name("myName", &input.my_name, NAME_MAX);
            let currency = f.currency("baseCurrency", &input.base_currency);
            f.finish()?;
            let currency = currency.ok_or_else(|| ApiError::internal("currency checked"))?;
            if self.store.household()?.is_some() {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    "this database already has a household",
                ));
            }
            let household = HouseholdRow {
                id: new_id(),
                name,
                base_currency_code: currency.code.to_string(),
                budget_mode: wire(BudgetMode::PerInstallment),
                locale: wire(input.locale.unwrap_or(Locale::EsCl)),
            };
            let me = MemberRow {
                id: new_id(),
                display_name: my_name,
                color: None,
            };
            self.store.write(|w| {
                w.insert_household(&household)?;
                w.insert_member(&household.id, &me)?;
                w.link_local_device(&me.id)?;
                Ok::<_, ApiError>(())
            })?;
            tracing::debug!(household = %household.id, member = %me.id, "household created");
            self.seed_household_defaults()?;
            household_dto(&household)
        })
    }

    pub fn settings_get(&self) -> Result<SettingsDto, ApiError> {
        command("settings_get", || settings_dto(&self.household_row()?))
    }

    /// The household, or `NOT_FOUND` before onboarding.
    pub(crate) fn household_row(&self) -> Result<HouseholdRow, ApiError> {
        self.store
            .household()?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "create the household first"))
    }

    /// The member this device belongs to, or `NOT_FOUND` before onboarding.
    pub(crate) fn me(&self) -> Result<String, ApiError> {
        self.store
            .local_member_id()?
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "create the household first"))
    }
}

fn household_dto(h: &HouseholdRow) -> Result<HouseholdDto, ApiError> {
    Ok(HouseholdDto {
        id: h.id.clone(),
        name: h.name.clone(),
        base_currency: h.base_currency_code.clone(),
        locale: from_wire(&h.locale)?,
    })
}

fn settings_dto(h: &HouseholdRow) -> Result<SettingsDto, ApiError> {
    Ok(SettingsDto {
        base_currency: h.base_currency_code.clone(),
        locale: from_wire(&h.locale)?,
        budget_mode: from_wire(&h.budget_mode)?,
    })
}

fn member_dto(m: MemberRow) -> MemberDto {
    MemberDto {
        id: m.id,
        display_name: m.display_name,
        color: m.color,
    }
}
