//! Planned items: an Income or Expense the user expects on a schedule, such
//! as salary every month on the 27th, rent on the 1st, or insurance every
//! year in June (CONTEXT.md).
//!
//! Expected occurrences are calculated from the schedule and never stored
//! (sync-design §1.1, §1.7). [`PlannedItem::occurrences_between`] is the one
//! place that expands a schedule into dates; Spendable, the Period view, and
//! later fulfilment by imported Transactions (ticket 09) and the Forecast
//! (ticket 11) all build on it.
//!
//! A Planned item that occurs less often than monthly is spread as a
//! Set-aside by default: its yearly total divided by 12 is subtracted from
//! every Period's Spendable, and its own occurrences are left out of
//! Spendable so the Period it falls due in isn't hit twice.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::categories::{self, CategoryKind};
use crate::date::{days_in_month, Date};
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::period::Period;
use crate::store::{Store, SyncedRow};

/// When a Planned item is expected.
///
/// A day past the end of a short month falls on that month's last day, so
/// "monthly on the 31st" is expected on 30 November and 28 (or 29) February,
/// and "yearly on 29 February" on 28 February outside leap years.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    /// On `day` of every `every_months`-th month, counted from the month of
    /// the start date: 1 is every month, 3 every quarter.
    Monthly { day: u32, every_months: u32 },
    /// Once a year, on `day` of `month` (1 = January).
    Yearly { month: u32, day: u32 },
}

impl Schedule {
    /// Whether it occurs every month (rather than less often).
    pub fn is_monthly(self) -> bool {
        self.months_between_occurrences() == 1
    }

    /// 1 for monthly, N for every N months, 12 for yearly.
    pub fn months_between_occurrences(self) -> u32 {
        match self {
            Schedule::Monthly { every_months, .. } => every_months,
            Schedule::Yearly { .. } => 12,
        }
    }

    /// Whether it falls in `period`, for an item starting in `start`.
    fn falls_in(self, start: Period, period: Period) -> bool {
        match self {
            Schedule::Monthly { every_months, .. } => {
                (month_index(period) - month_index(start)).rem_euclid(i64::from(every_months)) == 0
            }
            Schedule::Yearly { month, .. } => period.first_day().month() == month,
        }
    }

    fn day(self) -> u32 {
        match self {
            Schedule::Monthly { day, .. } | Schedule::Yearly { day, .. } => day,
        }
    }

    /// The date it falls on in `period` (whether or not it falls in it).
    fn date_in(self, period: Period) -> Date {
        let first = period.first_day();
        let day = self.day().min(days_in_month(first.year(), first.month()));
        Date::new(first.year(), first.month(), day).expect("clamped to the month")
    }

    /// The `rrule` column (sync-design §1.3), e.g. `FREQ=MONTHLY;BYMONTHDAY=27`
    /// or `FREQ=YEARLY;BYMONTH=6;BYMONTHDAY=15`. The start date is the
    /// rule's DTSTART.
    pub fn to_rrule(self) -> String {
        match self {
            Schedule::Monthly {
                day,
                every_months: 1,
            } => format!("FREQ=MONTHLY;BYMONTHDAY={day}"),
            Schedule::Monthly { day, every_months } => {
                format!("FREQ=MONTHLY;INTERVAL={every_months};BYMONTHDAY={day}")
            }
            Schedule::Yearly { month, day } => {
                format!("FREQ=YEARLY;BYMONTH={month};BYMONTHDAY={day}")
            }
        }
    }

    /// Parse what [`Schedule::to_rrule`] writes.
    pub fn from_rrule(rrule: &str) -> Result<Schedule> {
        let bad = || Error::Codec(format!("unsupported Planned item schedule {rrule:?}"));
        let mut freq = None;
        let (mut interval, mut month, mut day) = (1, None, None);
        for part in rrule.split(';') {
            let (key, value) = part.split_once('=').ok_or_else(bad)?;
            match key {
                "FREQ" => freq = Some(value),
                "INTERVAL" => interval = value.parse().map_err(|_| bad())?,
                "BYMONTH" => month = Some(value.parse().map_err(|_| bad())?),
                "BYMONTHDAY" => day = Some(value.parse().map_err(|_| bad())?),
                _ => return Err(bad()),
            }
        }
        match (freq, month, day) {
            (Some("MONTHLY"), None, Some(day)) => Ok(Schedule::Monthly {
                day,
                every_months: interval,
            }),
            (Some("YEARLY"), Some(month), Some(day)) if interval == 1 => {
                Ok(Schedule::Yearly { month, day })
            }
            _ => Err(bad()),
        }
    }
}

/// Months since year 0, so two Periods can be subtracted.
fn month_index(period: Period) -> i64 {
    let first = period.first_day();
    i64::from(first.year()) * 12 + i64::from(first.month()) - 1
}

/// One Planned item row (table `planned_item`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedItem {
    pub id: String,
    pub account_id: String,
    /// Its kind says whether the item is Income or an Expense.
    pub category_id: String,
    pub description: String,
    /// Signed, as it will appear on the Account: Income positive, an
    /// Expense negative.
    pub amount_minor: i64,
    pub schedule: Schedule,
    /// The first day it can occur. For "every N months", the month the
    /// counting starts from.
    pub start_date: Date,
    /// The last day it can occur; `None` until it is ended.
    pub end_date: Option<Date>,
    /// Spread into monthly Set-asides instead of hitting the Period it falls
    /// due in. Only for items less frequent than monthly.
    pub set_aside: bool,
}

impl PlannedItem {
    /// The dates the item is expected on within `from..=to`, in order,
    /// limited to its start and end dates. These are the raw occurrences of
    /// the schedule: occurrences that are fulfilled, skipped or moved
    /// (ticket 09) are not taken into account here.
    pub fn occurrences_between(&self, from: Date, to: Date) -> Vec<Date> {
        let from = from.max(self.start_date);
        let to = self.end_date.map_or(to, |end| to.min(end));
        let mut out = Vec::new();
        if from > to {
            return out;
        }
        let start = Period::containing(self.start_date);
        let mut period = Period::containing(from);
        while period.first_day() <= to {
            if self.schedule.falls_in(start, period) {
                let date = self.schedule.date_in(period);
                if from <= date && date <= to {
                    out.push(date);
                }
            }
            period = period.next();
        }
        out
    }

    /// The item's Set-aside for `period`: its yearly total divided by 12,
    /// signed like the amount (negative for an Expense) and rounded down, so
    /// enough is set aside for an Expense. 0 unless the item is spread as
    /// Set-asides and `period` overlaps its start and end dates.
    pub fn set_aside_for(&self, period: Period) -> i64 {
        let live = self.start_date <= period.last_day()
            && self.end_date.is_none_or(|end| period.first_day() <= end);
        if !self.set_aside || !live {
            return 0;
        }
        // amount × (12 / N occurrences a year) / 12
        self.amount_minor
            .div_euclid(i64::from(self.schedule.months_between_occurrences()))
    }
}

impl SyncedRow for PlannedItem {
    const ENTITY: &'static str = "planned_item";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO planned_item (id, account_id, category_id, description, amount_minor,
                                       rrule, start_date, end_date, set_aside, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)
             ON CONFLICT(id) DO UPDATE SET
               account_id = excluded.account_id, category_id = excluded.category_id,
               description = excluded.description, amount_minor = excluded.amount_minor,
               rrule = excluded.rrule, start_date = excluded.start_date,
               end_date = excluded.end_date, set_aside = excluded.set_aside,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.account_id,
                self.category_id,
                self.description,
                self.amount_minor,
                self.schedule.to_rrule(),
                self.start_date.to_string(),
                self.end_date.map(|d| d.to_string()),
                self.set_aside,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// What the user enters to create a Planned item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPlannedItem {
    pub account_id: String,
    pub category_id: String,
    pub description: String,
    /// Signed: Income positive, an Expense negative.
    pub amount_minor: i64,
    pub schedule: Schedule,
    pub start_date: Date,
    pub end_date: Option<Date>,
    /// `None` = the default: spread as Set-asides when the schedule is less
    /// often than monthly.
    pub set_aside: Option<bool>,
}

/// Create a Planned item.
pub fn create(store: &mut Store, new: NewPlannedItem) -> Result<PlannedItem> {
    store.write(|w| {
        let item = PlannedItem {
            id: uuid::Uuid::now_v7().to_string(),
            account_id: new.account_id,
            category_id: new.category_id,
            description: new.description.trim().to_owned(),
            amount_minor: new.amount_minor,
            set_aside: new.set_aside.unwrap_or(!new.schedule.is_monthly()),
            schedule: new.schedule,
            start_date: new.start_date,
            end_date: new.end_date,
        };
        validate(w.conn(), &item)?;
        w.upsert(&item)?;
        Ok(item)
    })
}

/// Save an edited Planned item: every field can change, including its end
/// date and whether it is spread as Set-asides.
pub fn update(store: &mut Store, item: PlannedItem) -> Result<PlannedItem> {
    store.write(|w| {
        find_live(w.conn(), &item.id)?;
        let item = PlannedItem {
            description: item.description.trim().to_owned(),
            ..item
        };
        validate(w.conn(), &item)?;
        w.upsert(&item)?;
        Ok(item)
    })
}

/// End a Planned item: `last_day` is the last day it can occur. Its earlier
/// occurrences and Set-asides still count in their Periods.
pub fn end(store: &mut Store, id: &str, last_day: Date) -> Result<PlannedItem> {
    store.write(|w| {
        let item = PlannedItem {
            end_date: Some(last_day),
            ..find_live(w.conn(), id)?
        };
        validate(w.conn(), &item)?;
        w.upsert(&item)?;
        Ok(item)
    })
}

/// Delete a Planned item (a soft delete), e.g. one entered by mistake. To
/// stop one but keep its past occurrences, [`end`] it instead.
pub fn delete(store: &mut Store, id: &str) -> Result<()> {
    store.write(|w| {
        w.delete::<PlannedItem>(id)?;
        Ok(())
    })
}

/// Every live Planned item, oldest first.
pub fn list(store: &Store) -> Result<Vec<PlannedItem>> {
    live_items(store.conn())
}

fn find_live(conn: &Connection, id: &str) -> Result<PlannedItem> {
    live_items(conn)?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| Error::NotFound {
            entity: PlannedItem::ENTITY,
            id: id.to_owned(),
        })
}

/// The rules for a Planned item the user saves.
fn validate(conn: &Connection, item: &PlannedItem) -> Result<()> {
    if item.description.is_empty() {
        return Err(Error::invalid("A Planned item needs a description."));
    }
    if !account_is_live(conn, &item.account_id)? {
        return Err(Error::invalid("The Planned item's Account doesn't exist."));
    }
    let category = categories::live_categories(conn)?
        .into_iter()
        .find(|c| c.id == item.category_id)
        .ok_or_else(|| Error::invalid("The Planned item's Category doesn't exist."))?;
    match category.kind {
        CategoryKind::Income if item.amount_minor <= 0 => {
            return Err(Error::invalid(
                "An Income Planned item brings money in: its amount must be more than zero.",
            ))
        }
        CategoryKind::Expense if item.amount_minor >= 0 => {
            return Err(Error::invalid(
                "An Expense Planned item takes money out: its amount must be below zero.",
            ))
        }
        _ => {}
    }
    validate_schedule(item.schedule)?;
    if item.set_aside && item.schedule.is_monthly() {
        return Err(Error::invalid(
            "Only Planned items that occur less often than monthly can be spread as Set-asides.",
        ));
    }
    if item.end_date.is_some_and(|end| end < item.start_date) {
        return Err(Error::invalid(
            "A Planned item can't end before its start date.",
        ));
    }
    Ok(())
}

fn validate_schedule(schedule: Schedule) -> Result<()> {
    match schedule {
        Schedule::Monthly { day, every_months } => {
            if !(1..=31).contains(&day) {
                return Err(Error::invalid("The day of the month must be from 1 to 31."));
            }
            if !(1..=120).contains(&every_months) {
                return Err(Error::invalid(
                    "A Planned item occurs every 1 to 120 months.",
                ));
            }
        }
        Schedule::Yearly { month, day } => {
            // 2024 is a leap year, so 29 February is allowed.
            if Date::new(2024, month, day).is_none() {
                return Err(Error::invalid(
                    "Enter a real date for a yearly Planned item.",
                ));
            }
        }
    }
    Ok(())
}

fn live_items(conn: &Connection) -> Result<Vec<PlannedItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, category_id, description, amount_minor, rrule, start_date,
                end_date, set_aside
         FROM planned_item WHERE deleted = 0 ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(StoredItem {
            id: r.get(0)?,
            account_id: r.get(1)?,
            category_id: r.get(2)?,
            description: r.get(3)?,
            amount_minor: r.get(4)?,
            rrule: r.get(5)?,
            start_date: r.get(6)?,
            end_date: r.get(7)?,
            set_aside: r.get(8)?,
        })
    })?;
    rows.map(|row| {
        let row = row?;
        Ok(PlannedItem {
            schedule: Schedule::from_rrule(&row.rrule)?,
            start_date: row.start_date.parse()?,
            end_date: row.end_date.map(|e| e.parse()).transpose()?,
            id: row.id,
            account_id: row.account_id,
            category_id: row.category_id,
            description: row.description,
            amount_minor: row.amount_minor,
            set_aside: row.set_aside,
        })
    })
    .collect()
}

/// One expected occurrence of a Planned item: calculated from its schedule,
/// never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    pub planned_item_id: String,
    pub description: String,
    pub account_id: String,
    pub category_id: String,
    /// Income or Expense, from the item's Category.
    pub kind: CategoryKind,
    /// The date it is expected on.
    pub due_date: Date,
    /// The planned amount, signed as it will appear on the Account.
    pub amount_minor: i64,
    /// The item is spread as Set-asides, so this occurrence is left out of
    /// Spendable (the Set-asides already cover it).
    pub set_aside: bool,
}

/// The expected occurrences of every live Planned item on a live Account
/// within `from..=to`, ordered by date. This is where fulfilment (ticket
/// 09: leave out fulfilled and skipped occurrences, move "arrives later"
/// ones) and the Forecast (ticket 11) plug in.
pub fn expected_occurrences(store: &Store, from: Date, to: Date) -> Result<Vec<Occurrence>> {
    let conn = store.conn();
    let mut out = Vec::new();
    for item in live_items(conn)? {
        if !account_is_live(conn, &item.account_id)? {
            continue;
        }
        let kind = kind_of(conn, &item)?;
        for due_date in item.occurrences_between(from, to) {
            out.push(Occurrence {
                planned_item_id: item.id.clone(),
                description: item.description.clone(),
                account_id: item.account_id.clone(),
                category_id: item.category_id.clone(),
                kind,
                due_date,
                amount_minor: item.amount_minor,
                set_aside: item.set_aside,
            });
        }
    }
    out.sort_by(|a, b| (a.due_date, &a.description).cmp(&(b.due_date, &b.description)));
    Ok(out)
}

/// One Planned item's Set-aside for a Period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAside {
    pub planned_item_id: String,
    pub description: String,
    pub category_id: String,
    pub schedule: Schedule,
    /// The item's planned amount per occurrence.
    pub item_amount_minor: i64,
    /// Its share for this Period, signed like the amount (negative for an
    /// Expense).
    pub amount_minor: i64,
}

/// A Period's expected occurrences and Set-asides: what the Period view
/// shows and what Spendable adds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeriodPlan {
    pub period: Period,
    /// Every expected occurrence in the Period, by date, including those of
    /// items spread as Set-asides (marked `set_aside`).
    pub occurrences: Vec<Occurrence>,
    pub set_asides: Vec<SetAside>,
}

impl PeriodPlan {
    /// Expected Income in the Period, leaving out items spread as Set-asides.
    pub fn expected_income_minor(&self) -> i64 {
        self.not_spread(CategoryKind::Income)
    }

    /// Expected Expenses in the Period (negative), leaving out items spread
    /// as Set-asides.
    pub fn expected_expenses_minor(&self) -> i64 {
        self.not_spread(CategoryKind::Expense)
    }

    /// The sum of the Period's Set-asides, signed (negative for Expenses).
    pub fn set_asides_minor(&self) -> i64 {
        self.set_asides.iter().map(|s| s.amount_minor).sum()
    }

    fn not_spread(&self, kind: CategoryKind) -> i64 {
        self.occurrences
            .iter()
            .filter(|o| o.kind == kind && !o.set_aside)
            .map(|o| o.amount_minor)
            .sum()
    }
}

/// The expected occurrences and Set-asides of `period`.
pub fn for_period(store: &Store, period: Period) -> Result<PeriodPlan> {
    let conn = store.conn();
    let mut set_asides = Vec::new();
    for item in live_items(conn)? {
        let amount_minor = item.set_aside_for(period);
        if amount_minor == 0 || !account_is_live(conn, &item.account_id)? {
            continue;
        }
        set_asides.push(SetAside {
            planned_item_id: item.id,
            description: item.description,
            category_id: item.category_id,
            schedule: item.schedule,
            item_amount_minor: item.amount_minor,
            amount_minor,
        });
    }
    Ok(PeriodPlan {
        period,
        occurrences: expected_occurrences(store, period.first_day(), period.last_day())?,
        set_asides,
    })
}

fn account_is_live(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM account WHERE id = ?1 AND deleted = 0)",
        params![id],
        |r| r.get(0),
    )?)
}

/// Income or Expense, from the item's Category. A Category deleted later
/// (perhaps on another Device) still says what the item was; if its row
/// hasn't arrived yet, the sign of the amount decides, which matches the
/// Category's kind by [`validate`].
fn kind_of(conn: &Connection, item: &PlannedItem) -> Result<CategoryKind> {
    let kind: Option<String> = conn
        .query_row(
            "SELECT kind FROM category WHERE id = ?1",
            params![item.category_id],
            |r| r.get(0),
        )
        .optional()?;
    match kind {
        Some(kind) => CategoryKind::parse(&kind),
        None if item.amount_minor > 0 => Ok(CategoryKind::Income),
        None => Ok(CategoryKind::Expense),
    }
}

/// A `planned_item` row as SQLite holds it.
struct StoredItem {
    id: String,
    account_id: String,
    category_id: String,
    description: String,
    amount_minor: i64,
    rrule: String,
    start_date: String,
    end_date: Option<String>,
    set_aside: bool,
}
