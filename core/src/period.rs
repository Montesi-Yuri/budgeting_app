//! Periods and Weeks (CONTEXT.md). A Period is a calendar month, from the
//! 1st to the last day. A Week runs Monday to Sunday and is cut at the
//! Period boundary, so a Period's first and last Weeks can be shorter than
//! seven days. Everything here is pure calendar arithmetic; "today" is
//! always passed in.

use crate::date::{days_in_month, Date};

/// A calendar month. Written and parsed as `YYYY-MM`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Period {
    year: i32,
    month: u32,
}

impl Period {
    /// The Period a date falls in.
    pub fn containing(date: Date) -> Period {
        Period {
            year: date.year(),
            month: date.month(),
        }
    }

    /// The Period before this one.
    pub fn previous(self) -> Period {
        Period::containing(self.first_day().add_days(-1))
    }

    /// The Period after this one.
    pub fn next(self) -> Period {
        Period::containing(self.last_day().add_days(1))
    }

    pub fn first_day(self) -> Date {
        Date::new(self.year, self.month, 1).expect("a Period's 1st is a date")
    }

    pub fn last_day(self) -> Date {
        Date::new(self.year, self.month, days_in_month(self.year, self.month))
            .expect("a Period's last day is a date")
    }

    /// Days from `today` to the end of the Period, counting today: 1 on the
    /// last day. `today` must be in this Period.
    pub fn days_left(self, today: Date) -> u32 {
        debug_assert_eq!(Period::containing(today), self);
        (today.days_until(self.last_day()) + 1) as u32
    }
}

impl std::fmt::Display for Period {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

impl std::str::FromStr for Period {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Period> {
        format!("{s}-01")
            .parse::<Date>()
            .ok()
            .filter(|_| s.len() == 7)
            .map(Period::containing)
            .ok_or_else(|| crate::Error::invalid(format!("{s:?} is not a Period (YYYY-MM).")))
    }
}

impl serde::Serialize for Period {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

/// A Week: Monday to Sunday, cut at the Period boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Week {
    first_day: Date,
    last_day: Date,
}

impl Week {
    /// The Week a date falls in, cut to that date's Period.
    pub fn containing(date: Date) -> Week {
        let period = Period::containing(date);
        let monday = date.add_days(-i64::from(date.days_from_monday()));
        let sunday = monday.add_days(6);
        Week {
            first_day: monday.max(period.first_day()),
            last_day: sunday.min(period.last_day()),
        }
    }

    pub fn first_day(self) -> Date {
        self.first_day
    }

    pub fn last_day(self) -> Date {
        self.last_day
    }

    /// Days from `today` to the end of the Week, counting today. `today`
    /// must be in this Week.
    pub fn days_left(self, today: Date) -> u32 {
        debug_assert!(self.first_day <= today && today <= self.last_day);
        (today.days_until(self.last_day) + 1) as u32
    }
}
