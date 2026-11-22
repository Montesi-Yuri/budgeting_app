//! Categorisation Rules: conditions applied automatically to Transactions
//! when they are imported (CONTEXT.md "Rule"). A Rule matches description
//! text (case-insensitive "contains"), optionally limited to one Account
//! and/or one direction, and either assigns a Category or marks the
//! Transaction as part of a Transfer ([`RuleAction`]). Rules are kept in an order
//! the user can change; the first matching Rule wins. A Category set by the
//! user by hand is never changed by a Rule.
//!
//! A Rule whose Category has been deleted is **broken**: it stays, is shown
//! as broken, and is not applied until another Category is picked.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Store, SyncedRow};
use crate::transactions::{self, CategorySource, Transaction, TransactionView};

/// Money into the Account (a positive amount) or out of it (negative).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    In,
    Out,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::In => "in",
            Direction::Out => "out",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "in" => Ok(Direction::In),
            "out" => Ok(Direction::Out),
            other => Err(Error::Codec(format!("unknown Rule direction {other:?}"))),
        }
    }

    /// Whether a signed amount goes this way.
    pub fn of(self, amount_minor: i64) -> bool {
        match self {
            Direction::In => amount_minor > 0,
            Direction::Out => amount_minor < 0,
        }
    }
}

/// What a matching Rule does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// Assign the Rule's Category.
    #[default]
    Categorise,
    /// Mark the Transaction as part of a Transfer (confirmed straight away,
    /// with one side only if the other side is missing; see
    /// [`crate::transfers`]). It has no Category and leaves the
    /// Transaction's Category alone.
    Transfer,
}

impl RuleAction {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleAction::Categorise => "categorise",
            RuleAction::Transfer => "transfer",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "categorise" => Ok(RuleAction::Categorise),
            "transfer" => Ok(RuleAction::Transfer),
            other => Err(Error::Codec(format!("unknown Rule action {other:?}"))),
        }
    }
}

/// One Rule row (table `rule`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    /// First match wins, lowest first. A REAL, so moving one Rule writes
    /// only that row.
    pub position: f64,
    /// Looked for in the description, ignoring case.
    pub match_text: String,
    /// Only Transactions on this Account; `None` = any Account.
    pub account_id: Option<String>,
    /// Only money in or only money out; `None` = either.
    pub direction: Option<Direction>,
    pub action: RuleAction,
    /// The Category to assign (for [`RuleAction::Categorise`]).
    pub category_id: Option<String>,
}

impl SyncedRow for Rule {
    const ENTITY: &'static str = "rule";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO rule (id, position, match_text, account_id, direction, action,
                               category_id, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)
             ON CONFLICT(id) DO UPDATE SET
               position = excluded.position, match_text = excluded.match_text,
               account_id = excluded.account_id, direction = excluded.direction,
               action = excluded.action, category_id = excluded.category_id,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.position,
                self.match_text,
                self.account_id,
                self.direction.map(Direction::as_str),
                self.action.as_str(),
                self.category_id,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// What the user enters to create a Rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRule {
    pub match_text: String,
    pub account_id: Option<String>,
    pub direction: Option<Direction>,
    /// Categorise (the default) or mark as Transfer.
    #[serde(default)]
    pub action: RuleAction,
    /// Required to categorise; ignored for a Transfer Rule.
    pub category_id: Option<String>,
}

/// Create a Rule, placed after the others (so it is tried last).
pub fn create(store: &mut Store, new: NewRule) -> Result<Rule> {
    store.write(|w| {
        let position = live_rules(w.conn())?
            .iter()
            .map(|r| r.position)
            .fold(0.0, f64::max)
            + 1.0;
        let rule = validated(
            w.conn(),
            Rule {
                id: uuid::Uuid::now_v7().to_string(),
                position,
                match_text: new.match_text,
                account_id: new.account_id,
                direction: new.direction,
                action: new.action,
                category_id: new.category_id,
            },
        )?;
        w.upsert(&rule)?;
        Ok(rule)
    })
}

/// The fields the user can change on a Rule. Its place in the order stays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEdit {
    pub id: String,
    pub match_text: String,
    pub account_id: Option<String>,
    pub direction: Option<Direction>,
    #[serde(default)]
    pub action: RuleAction,
    pub category_id: Option<String>,
}

/// Save the user's edit of a Rule. Picking a live Category repairs a
/// broken Rule. Existing Transactions are not changed: see
/// [`recategorise_preview`].
pub fn update(store: &mut Store, edit: RuleEdit) -> Result<Rule> {
    store.write(|w| {
        let current = find_live(w.conn(), &edit.id)?;
        let rule = validated(
            w.conn(),
            Rule {
                match_text: edit.match_text,
                account_id: edit.account_id,
                direction: edit.direction,
                action: edit.action,
                category_id: edit.category_id,
                ..current
            },
        )?;
        w.upsert(&rule)?;
        Ok(rule)
    })
}

/// Delete a Rule (a soft delete). Transactions it categorised keep their
/// Category.
pub fn delete(store: &mut Store, id: &str) -> Result<()> {
    store.write(|w| w.delete::<Rule>(id))?;
    Ok(())
}

/// Move a Rule to `index` in the order (0 = tried first; past the end =
/// last). Only the moved Rule is written: it gets a position between its
/// new neighbours, so reordering on two Devices conflicts only for the same
/// Rule.
pub fn move_to(store: &mut Store, id: &str, index: usize) -> Result<Rule> {
    store.write(|w| {
        let moving = find_live(w.conn(), id)?;
        let others: Vec<f64> = live_rules(w.conn())?
            .into_iter()
            .filter(|r| r.id != moving.id)
            .map(|r| r.position)
            .collect();
        let index = index.min(others.len());
        let position = match (index.checked_sub(1).map(|i| others[i]), others.get(index)) {
            (None, None) => moving.position,
            (None, Some(&next)) => next - 1.0,
            (Some(prev), None) => prev + 1.0,
            (Some(prev), Some(&next)) => (prev + next) / 2.0,
        };
        let rule = Rule { position, ..moving };
        w.upsert(&rule)?;
        Ok(rule)
    })
}

fn find_live(conn: &Connection, id: &str) -> Result<Rule> {
    live_rules(conn)?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| Error::NotFound {
            entity: Rule::ENTITY,
            id: id.to_owned(),
        })
}

/// Check what the user entered and tidy the text.
fn validated(conn: &Connection, mut rule: Rule) -> Result<Rule> {
    rule.match_text = rule.match_text.trim().to_owned();
    if rule.match_text.is_empty() {
        return Err(Error::invalid(
            "A Rule needs some text to look for in the description.",
        ));
    }
    if let Some(account_id) = &rule.account_id {
        if !is_live(conn, "account", account_id)? {
            return Err(Error::invalid(
                "Choose an existing Account, or any Account.",
            ));
        }
    }
    match rule.action {
        RuleAction::Categorise => match &rule.category_id {
            Some(category_id) if is_live(conn, "category", category_id)? => {}
            _ => return Err(Error::invalid("Choose an existing Category.")),
        },
        RuleAction::Transfer => rule.category_id = None,
    }
    Ok(rule)
}

fn is_live(conn: &Connection, table: &str, id: &str) -> Result<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1 AND deleted = 0)");
    Ok(conn.query_row(&sql, [id], |r| r.get(0))?)
}

/// A Rule as shown in the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleView {
    #[serde(flatten)]
    pub rule: Rule,
    /// `None` when the Category has been deleted.
    pub category_name: Option<String>,
    /// `None` for any Account (or a deleted one).
    pub account_name: Option<String>,
    /// Its Category has been deleted: it is not applied.
    pub broken: bool,
}

/// Every live Rule in order (the order they are tried in).
pub fn list(store: &Store) -> Result<Vec<RuleView>> {
    let conn = store.conn();
    let mut stmt = conn.prepare(
        "SELECT r.id, c.name, a.name FROM rule r
         LEFT JOIN category c ON c.id = r.category_id AND c.deleted = 0
         LEFT JOIN account a ON a.id = r.account_id AND a.deleted = 0
         WHERE r.deleted = 0",
    )?;
    let names = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ),
            ))
        })?
        .collect::<rusqlite::Result<std::collections::HashMap<_, _>>>()?;
    Ok(live_rules(conn)?
        .into_iter()
        .map(|rule| {
            let (category_name, account_name) = names.get(&rule.id).cloned().unwrap_or_default();
            let broken = rule.action == RuleAction::Categorise && category_name.is_none();
            RuleView {
                rule,
                category_name,
                account_name,
                broken,
            }
        })
        .collect())
}

/// Every live Rule, in the order they are tried.
pub(crate) fn live_rules(conn: &Connection) -> Result<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT id, position, match_text, account_id, direction, action, category_id
         FROM rule WHERE deleted = 0 ORDER BY position, id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, f64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
        ))
    })?;
    rows.map(|row| {
        let (id, position, match_text, account_id, direction, action, category_id) = row?;
        Ok(Rule {
            id,
            position,
            match_text,
            account_id,
            direction: direction.as_deref().map(Direction::parse).transpose()?,
            action: RuleAction::parse(&action)?,
            category_id,
        })
    })
    .collect()
}

impl Rule {
    /// Whether this Rule's condition holds for `txn`: the description
    /// contains the text (ignoring case), and the Account and direction
    /// filters, when set, agree.
    pub fn matches(&self, txn: &Transaction) -> bool {
        self.account_id
            .as_ref()
            .is_none_or(|a| *a == txn.account_id)
            && self.direction.is_none_or(|d| d.of(txn.amount_minor))
            && txn
                .description
                .to_lowercase()
                .contains(&self.match_text.to_lowercase())
    }
}

/// The Rules that can be applied, in order: every live Rule that isn't
/// broken. Load them once, then use [`first_match`] / [`categorise`] on
/// each Transaction.
pub fn applicable(conn: &Connection) -> Result<Vec<Rule>> {
    let live_categories: std::collections::HashSet<String> = conn
        .prepare("SELECT id FROM category WHERE deleted = 0")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(live_rules(conn)?
        .into_iter()
        .filter(|r| match r.action {
            RuleAction::Categorise => r
                .category_id
                .as_ref()
                .is_some_and(|c| live_categories.contains(c)),
            RuleAction::Transfer => true,
        })
        .collect())
}

/// The first of `rules` (in order) that matches `txn`.
pub fn first_match<'a>(rules: &'a [Rule], txn: &Transaction) -> Option<&'a Rule> {
    rules.iter().find(|r| r.matches(txn))
}

/// Give `txn` the Category of the first matching Rule, marked as set by a
/// Rule. A Category set by the user is never changed. Returns the Rule that
/// was applied, if any.
pub fn categorise<'a>(rules: &'a [Rule], txn: &mut Transaction) -> Option<&'a Rule> {
    if txn.category_source == Some(CategorySource::User) {
        return None;
    }
    let rule = first_match(rules, txn)?;
    match rule.action {
        RuleAction::Categorise => {
            txn.category_id = rule.category_id.clone();
            txn.category_source = Some(CategorySource::Rule);
        }
        RuleAction::Transfer => {} // the caller makes the Transfer
    }
    Some(rule)
}

/// The existing Transactions that Rule `rule_id` would re-categorise, newest
/// first, for the user to confirm after adding or editing it: those it is
/// the first applicable Rule for, whose Category it would change.
/// Transactions with a Category set by the user are never listed, and a
/// broken Rule lists nothing. Nothing is changed.
pub fn recategorise_preview(store: &Store, rule_id: &str) -> Result<Vec<TransactionView>> {
    would_recategorise(store.conn(), rule_id)
}

/// Apply Rule `rule_id` to the Transactions in `txn_ids` that the user
/// confirmed from [`recategorise_preview`], in one change. Each one is
/// checked again first, so a Transaction the user has categorised by hand in
/// the meantime is left alone. Returns how many were changed.
pub fn recategorise(store: &mut Store, rule_id: &str, txn_ids: &[String]) -> Result<usize> {
    store.write(|w| {
        let rules = applicable(w.conn())?;
        let mut changed = 0;
        for view in would_recategorise(w.conn(), rule_id)? {
            if !txn_ids.contains(&view.transaction.id) {
                continue;
            }
            let mut txn = view.transaction;
            categorise(&rules, &mut txn);
            w.upsert(&txn)?;
            changed += 1;
        }
        Ok(changed)
    })
}

fn would_recategorise(conn: &Connection, rule_id: &str) -> Result<Vec<TransactionView>> {
    find_live(conn, rule_id)?;
    let rules = applicable(conn)?;
    let Some(rule) = rules.iter().find(|r| r.id == rule_id) else {
        return Ok(Vec::new()); // broken
    };
    if rule.action != RuleAction::Categorise {
        return Ok(Vec::new()); // marks new Transactions only
    }
    Ok(transactions::list_live(conn)?
        .into_iter()
        .filter(|view| {
            let txn = &view.transaction;
            txn.category_source != Some(CategorySource::User)
                && first_match(&rules, txn).is_some_and(|r| r.id == rule.id)
                && txn.category_id != rule.category_id
        })
        .collect())
}
