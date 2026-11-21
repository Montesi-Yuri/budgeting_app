//! Categories: labels saying what an Expense or Income was for. Each is an
//! income or an expense Category, with at most two levels: a parent and its
//! children, which have the parent's kind (CONTEXT.md).

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hlc::Hlc;
use crate::store::{Store, SyncedRow};

/// Whether a Transaction in this Category is Income or an Expense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryKind {
    Income,
    Expense,
}

impl CategoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CategoryKind::Income => "income",
            CategoryKind::Expense => "expense",
        }
    }

    pub(crate) fn parse(s: &str) -> Result<Self> {
        match s {
            "income" => Ok(CategoryKind::Income),
            "expense" => Ok(CategoryKind::Expense),
            other => Err(Error::Codec(format!("unknown Category kind {other:?}"))),
        }
    }
}

/// One Category row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub kind: CategoryKind,
    /// The parent Category; `None` for a top-level Category.
    pub parent_id: Option<String>,
    pub color: Option<String>,
    /// Display order among siblings. A REAL, so moving one Category writes
    /// only that row.
    pub position: f64,
}

impl SyncedRow for Category {
    const ENTITY: &'static str = "category";

    fn id(&self) -> &str {
        &self.id
    }

    fn save(&self, conn: &Connection, hlc: &Hlc) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO category (id, name, kind, parent_id, color, position, updated_hlc, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name, kind = excluded.kind, parent_id = excluded.parent_id,
               color = excluded.color, position = excluded.position,
               updated_hlc = excluded.updated_hlc, deleted = 0",
            params![
                self.id,
                self.name,
                self.kind.as_str(),
                self.parent_id,
                self.color,
                self.position,
                hlc.to_string()
            ],
        )?;
        Ok(())
    }
}

/// What the user enters to create a Category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewCategory {
    pub name: String,
    pub kind: CategoryKind,
    /// Create it as a child of this top-level Category.
    pub parent_id: Option<String>,
}

/// Create a Category, placed after its siblings.
pub fn create(store: &mut Store, new: NewCategory) -> Result<Category> {
    let name = valid_name(&new.name)?;
    store.write(|w| {
        let all = live_categories(w.conn())?;
        if let Some(parent_id) = &new.parent_id {
            let parent = all
                .iter()
                .find(|c| &c.id == parent_id)
                .ok_or_else(|| Error::invalid("The parent Category doesn't exist."))?;
            if parent.parent_id.is_some() {
                return Err(Error::invalid(
                    "Categories have at most two levels: a child Category can't have children.",
                ));
            }
            if parent.kind != new.kind {
                return Err(Error::invalid(
                    "A child Category has the same kind (income or expense) as its parent.",
                ));
            }
        }
        let position = all
            .iter()
            .filter(|c| c.parent_id == new.parent_id)
            .map(|c| c.position)
            .fold(0.0, f64::max)
            + 1.0;
        let category = Category {
            id: uuid::Uuid::now_v7().to_string(),
            name,
            kind: new.kind,
            parent_id: new.parent_id.clone(),
            color: None,
            position,
        };
        w.upsert(&category)?;
        Ok(category)
    })
}

/// Give a Category a new name. Its kind, parent and place stay the same.
pub fn rename(store: &mut Store, id: &str, name: &str) -> Result<Category> {
    let name = valid_name(name)?;
    store.write(|w| {
        let category = Category {
            name,
            ..find_live(w.conn(), id)?
        };
        w.upsert(&category)?;
        Ok(category)
    })
}

/// Move a Category to `index` among its siblings (0 = first; past the end =
/// last). Only the moved Category is written: it gets a position between its
/// new neighbours, so moves on two Devices conflict only for the same row.
pub fn move_to(store: &mut Store, id: &str, index: usize) -> Result<Category> {
    store.write(|w| {
        let moving = find_live(w.conn(), id)?;
        let siblings: Vec<f64> = live_categories(w.conn())?
            .into_iter()
            .filter(|c| c.parent_id == moving.parent_id && c.id != moving.id)
            .map(|c| c.position)
            .collect();
        let index = index.min(siblings.len());
        let position = match (
            index.checked_sub(1).map(|i| siblings[i]),
            siblings.get(index),
        ) {
            (None, None) => moving.position,
            (None, Some(&next)) => next - 1.0,
            (Some(prev), None) => prev + 1.0,
            (Some(prev), Some(&next)) => (prev + next) / 2.0,
        };
        let category = Category { position, ..moving };
        w.upsert(&category)?;
        Ok(category)
    })
}

/// Delete a Category (a soft delete) and, for a parent, its children, in one
/// change. Transactions keep their `category_id` (sync-design §6) and are
/// read as Uncategorised while it points to a deleted Category.
pub fn delete(store: &mut Store, id: &str) -> Result<()> {
    store.write(|w| {
        let children: Vec<String> = live_categories(w.conn())?
            .into_iter()
            .filter(|c| c.parent_id.as_deref() == Some(id))
            .map(|c| c.id)
            .collect();
        w.delete::<Category>(id)?;
        for child in children {
            w.delete::<Category>(&child)?;
        }
        Ok(())
    })
}

fn find_live(conn: &Connection, id: &str) -> Result<Category> {
    live_categories(conn)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| Error::NotFound {
            entity: Category::ENTITY,
            id: id.to_owned(),
        })
}

fn valid_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::invalid("A Category needs a name."));
    }
    Ok(name.to_owned())
}

/// Every live Category in display order: each top-level Category by
/// position, followed by its children by position.
pub fn list(store: &Store) -> Result<Vec<Category>> {
    live_categories(store.conn())
}

pub(crate) fn live_categories(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, parent_id, color, position FROM category
         WHERE deleted = 0 ORDER BY position, id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, f64>(5)?,
        ))
    })?;
    let flat = rows
        .map(|row| {
            let (id, name, kind, parent_id, color, position) = row?;
            Ok(Category {
                id,
                name,
                kind: CategoryKind::parse(&kind)?,
                parent_id,
                color,
                position,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(tree_order(flat))
}

/// Parents in order, each followed by its children. `flat` is already sorted
/// by position. A child whose parent is missing (e.g. deleted on another
/// Device) is shown at the top level.
fn tree_order(flat: Vec<Category>) -> Vec<Category> {
    let is_live = |id: &str| flat.iter().any(|c| c.id == id);
    let (tops, children): (Vec<_>, Vec<_>) = flat
        .iter()
        .cloned()
        .partition(|c| c.parent_id.as_deref().is_none_or(|p| !is_live(p)));
    let mut out = Vec::with_capacity(flat.len());
    for parent in tops {
        let id = parent.id.clone();
        out.push(parent);
        out.extend(
            children
                .iter()
                .filter(|c| c.parent_id.as_deref() == Some(&id))
                .cloned(),
        );
    }
    out
}
