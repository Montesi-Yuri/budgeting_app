//! Calendar dates, stored everywhere as `YYYY-MM-DD` text.

/// Whether `s` is a real calendar date written as `YYYY-MM-DD`.
pub fn is_valid(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let num = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = &s[range];
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())
            .flatten()
    };
    let (Some(year), Some(month), Some(day)) = (num(0..4), num(5..7), num(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days_in_month).contains(&day)
}

/// Whether `year` has a 29 February.
pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// The number of days in a month (`month` 1..=12).
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => panic!("month {month} out of range"),
    }
}

/// A calendar date. It is written and parsed as `YYYY-MM-DD` and orders
/// chronologically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u32,
    day: u32,
}

impl Date {
    /// `None` unless it is a real calendar date in years 1..=9999.
    pub fn new(year: i32, month: u32, day: u32) -> Option<Date> {
        ((1..=9999).contains(&year)
            && (1..=12).contains(&month)
            && (1..=days_in_month(year, month)).contains(&day))
        .then_some(Date { year, month, day })
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u32 {
        self.month
    }

    pub fn day(self) -> u32 {
        self.day
    }

    /// Days since 1970-01-01 (negative before it).
    fn day_number(self) -> i64 {
        // Howard Hinnant's days_from_civil.
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn from_day_number(z: i64) -> Date {
        // Howard Hinnant's civil_from_days.
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Date { year, month, day }
    }

    /// The date `days` days later (earlier when negative).
    pub fn add_days(self, days: i64) -> Date {
        Date::from_day_number(self.day_number() + days)
    }

    /// Whole days from `self` to `later` (negative if `later` is earlier).
    pub fn days_until(self, later: Date) -> i64 {
        later.day_number() - self.day_number()
    }

    /// Days since the most recent Monday: 0 for a Monday, 6 for a Sunday.
    pub fn days_from_monday(self) -> u32 {
        // 1970-01-01 was a Thursday (3 days after a Monday).
        (self.day_number() + 3).rem_euclid(7) as u32
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl std::str::FromStr for Date {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Date> {
        let invalid = || crate::Error::invalid(format!("{s:?} is not a date (YYYY-MM-DD)."));
        if !is_valid(s) {
            return Err(invalid());
        }
        let num = |r: std::ops::Range<usize>| s[r].parse::<u32>().map_err(|_| invalid());
        Date::new(num(0..4)? as i32, num(5..7)?, num(8..10)?).ok_or_else(invalid)
    }
}

impl serde::Serialize for Date {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Date {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}
