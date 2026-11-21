//! Accounts: a current account, a savings account or a credit card, each
//! with an Opening balance on a date (CONTEXT.md).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::date;
use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Store, SyncedRow, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Current,
    Savings,
    CreditCard,
}

impl AccountKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AccountKind::Current => "current",
            AccountKind::Savings => "savings",
            AccountKind::CreditCard => "credit_card",
        }
    }

    fn parse(s: &str) -> Result<Self> {
        match s {
            "current" => Ok(AccountKind::Current),
            "savings" => Ok(AccountKind::Savings),
            "credit_card" => Ok(AccountKind::CreditCard),
            other => Err(Error::Codec(format!("unknown Account kind {other:?}"))),
        }
    }
}

/// One Account row. Amounts are integer minor units of the Currency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub kind: AccountKind,
    /// The Opening balance, signed: credit card debt is negative.
    pub opening_minor: i64,
    /// The day the Opening balance is valid for, `YYYY-MM-DD`.
    pub opening_date: String,
    /// The main (current) Account, shown in the Forecast. At most one.
    pub is_main: bool,
}

impl SyncedRow for Account {
    const ENTITY: &'static str = "account";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO account (id, name, kind, opening_minor, opening_date, is_main, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name, kind = excluded.kind,
               opening_minor = excluded.opening_minor, opening_date = excluded.opening_date,
               is_main = excluded.is_main, updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.name,
                self.kind.as_str(),
                self.opening_minor,
                self.opening_date,
                self.is_main,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// What the user enters to create an Account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAccount {
    pub name: String,
    pub kind: AccountKind,
    pub opening_minor: i64,
    pub opening_date: String,
    pub is_main: bool,
}

/// An Account with its calculated balance (never stored, see sync-design §1.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountWithBalance {
    #[serde(flatten)]
    pub account: Account,
    /// Today's balance in minor units. For now the Opening balance, since
    /// there are no Transactions yet (ticket 02).
    pub balance_minor: i64,
}

/// Create an Account with a new UUIDv7 id. If it is marked as main, the
/// previous main Account is unmarked in the same change.
pub fn create(store: &mut Store, new: NewAccount) -> Result<Account> {
    let account = validated(Account {
        id: uuid::Uuid::now_v7().to_string(),
        name: new.name,
        kind: new.kind,
        opening_minor: new.opening_minor,
        opening_date: new.opening_date,
        is_main: new.is_main,
    })?;
    store.write(|w| save_with_single_main(w, &account))?;
    Ok(account)
}

/// Save an edited Account (rename, kind, Opening balance and its date, main).
pub fn update(store: &mut Store, account: Account) -> Result<Account> {
    let account = validated(account)?;
    store.write(|w| {
        if !live_accounts(w.conn())?.iter().any(|a| a.id == account.id) {
            return Err(Error::NotFound {
                entity: Account::ENTITY,
                id: account.id.clone(),
            });
        }
        save_with_single_main(w, &account)
    })?;
    Ok(account)
}

/// Delete an Account (a soft delete, so the deletion syncs).
pub fn delete(store: &mut Store, id: &str) -> Result<()> {
    store.write(|w| w.delete::<Account>(id))?;
    Ok(())
}

fn validated(mut account: Account) -> Result<Account> {
    account.name = account.name.trim().to_owned();
    if account.name.is_empty() {
        return Err(Error::invalid("An Account needs a name."));
    }
    if !date::is_valid(&account.opening_date) {
        return Err(Error::invalid(
            "The Opening balance date must be a valid date (YYYY-MM-DD).",
        ));
    }
    Ok(account)
}

fn save_with_single_main(w: &mut Writer<'_>, account: &Account) -> Result<()> {
    if account.is_main {
        for other in live_accounts(w.conn())? {
            if other.is_main && other.id != account.id {
                w.upsert(&Account {
                    is_main: false,
                    ..other
                })?;
            }
        }
    }
    w.upsert(account)?;
    Ok(())
}

/// Every live Account with its balance: the main Account first, then by name.
pub fn list(store: &Store) -> Result<Vec<AccountWithBalance>> {
    let accounts = live_accounts(store.conn())?;
    Ok(accounts
        .into_iter()
        .map(|account| AccountWithBalance {
            balance_minor: balance_minor(&account),
            account,
        })
        .collect())
}

fn balance_minor(account: &Account) -> i64 {
    account.opening_minor
}

fn live_accounts(conn: &Connection) -> Result<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, opening_minor, opening_date, is_main FROM account
         WHERE deleted = 0 ORDER BY is_main DESC, name COLLATE NOCASE, id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, bool>(5)?,
        ))
    })?;
    rows.map(|row| {
        let (id, name, kind, opening_minor, opening_date, is_main) = row?;
        Ok(Account {
            id,
            name,
            kind: AccountKind::parse(&kind)?,
            opening_minor,
            opening_date,
            is_main,
        })
    })
    .collect()
}
