//! Reading a bank CSV file with an Import profile: decoding, skipping the
//! lines before the header, and turning each line into a date, a signed
//! amount, a description and an optional balance. Pure: no database.

use super::{AmountMode, Encoding, ImportProfile};
use crate::date;

/// One line of the file after the header, as the profile reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedLine {
    /// The line number in the file (1 = the first line).
    pub line: usize,
    pub date: Option<String>,
    pub amount_minor: Option<i64>,
    pub description: String,
    pub balance_after_minor: Option<i64>,
    /// Why the line can't be imported; `None` when it can.
    pub error: Option<String>,
}

/// The file as the profile reads it.
#[derive(Debug, Clone, Default)]
pub(crate) struct ParsedFile {
    /// The first lines of the file as text, before anything is skipped.
    pub first_lines: Vec<String>,
    /// The header's column names.
    pub columns: Vec<String>,
    /// A problem with the whole file or profile (such as a missing column);
    /// when set, `lines` is empty.
    pub problem: Option<String>,
    pub lines: Vec<ParsedLine>,
}

const FIRST_LINES: usize = 12;

pub(crate) fn parse(profile: &ImportProfile, minor_digits: u32, file: &[u8]) -> ParsedFile {
    let text = match decode(profile.encoding, file) {
        Ok(text) => text,
        Err(problem) => {
            return ParsedFile {
                problem: Some(problem),
                ..ParsedFile::default()
            }
        }
    };
    let mut out = ParsedFile {
        first_lines: text.lines().take(FIRST_LINES).map(str::to_owned).collect(),
        ..ParsedFile::default()
    };
    if let Err(problem) = read_lines(profile, minor_digits, &text, &mut out) {
        out.problem = Some(problem);
        out.lines.clear();
    }
    out
}

fn decode(encoding: Encoding, file: &[u8]) -> Result<String, String> {
    match encoding {
        Encoding::Utf8 => {
            let bytes = file.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(file);
            String::from_utf8(bytes.to_vec()).map_err(|_| {
                "The file isn't valid UTF-8 text. Try the Windows-1252 encoding.".to_owned()
            })
        }
        Encoding::Windows1252 => Ok(encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(file)
            .0
            .into_owned()),
    }
}

fn read_lines(
    profile: &ImportProfile,
    minor_digits: u32,
    text: &str,
    out: &mut ParsedFile,
) -> Result<(), String> {
    let delimiter = single_char(&profile.delimiter)
        .filter(char::is_ascii)
        .ok_or("The separator must be a single character such as ; or ,")?;
    // Skip the lines before the header.
    let mut rest = text;
    for _ in 0..profile.skip_rows {
        match rest.find('\n') {
            Some(end) => rest = &rest[end + 1..],
            None => return Err("The file has fewer lines than the lines to skip.".to_owned()),
        }
    }
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter as u8)
        .has_headers(false)
        .flexible(true)
        .from_reader(rest.as_bytes());
    let mut records = reader.records();
    let header = match records.next() {
        Some(Ok(header)) => header,
        Some(Err(e)) => return Err(format!("The header can't be read: {e}")),
        None => return Err("The file has no header line.".to_owned()),
    };
    out.columns = header.iter().map(|c| c.trim().to_owned()).collect();

    let layout = Layout::new(profile, &out.columns)?;
    let reader = Reader {
        decimal_sep: single_char(&profile.decimal_sep).unwrap_or(','),
        thousands_sep: profile.thousands_sep.as_deref().and_then(single_char),
        minor_digits,
        date_format: &profile.date_format,
    };
    for record in records {
        let record = record.map_err(|e| format!("The file can't be read: {e}"))?;
        let line = record.position().map_or(0, |p| p.line() as usize) + profile.skip_rows as usize;
        if record.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        let cell = |index: usize| record.get(index).unwrap_or("").trim();
        // A line without a date (such as a footer or a note) is not a Transaction.
        if cell(layout.date).is_empty() {
            continue;
        }
        out.lines.push(reader.line(&layout, line, cell));
    }
    Ok(())
}

/// Where the profile's columns are in this file.
struct Layout {
    date: usize,
    amount: AmountColumns,
    description: Vec<usize>,
    balance: Option<usize>,
}

enum AmountColumns {
    Signed(usize),
    Split { debit: usize, credit: usize },
}

impl Layout {
    fn new(profile: &ImportProfile, columns: &[String]) -> Result<Layout, String> {
        let find = |what: &str, name: &str| -> Result<usize, String> {
            let name = name.trim();
            if name.is_empty() {
                return Err(format!("Choose the {what} column."));
            }
            columns
                .iter()
                .position(|c| c == name)
                .or_else(|| columns.iter().position(|c| c.eq_ignore_ascii_case(name)))
                .ok_or_else(|| format!("The file has no column \"{name}\" ({what})."))
        };
        let optional = |what: &str, name: &Option<String>| -> Result<usize, String> {
            find(what, name.as_deref().unwrap_or(""))
        };
        let amount = match profile.amount_mode {
            AmountMode::Signed => {
                AmountColumns::Signed(optional("amount", &profile.amount_column)?)
            }
            AmountMode::Split => AmountColumns::Split {
                debit: optional("debit", &profile.debit_column)?,
                credit: optional("credit", &profile.credit_column)?,
            },
        };
        if profile.description_columns.is_empty() {
            return Err("Choose at least one description column.".to_owned());
        }
        Ok(Layout {
            date: find("date", &profile.date_column)?,
            amount,
            description: profile
                .description_columns
                .iter()
                .map(|name| find("description", name))
                .collect::<Result<_, _>>()?,
            balance: match &profile.balance_column {
                Some(name) if !name.trim().is_empty() => Some(find("balance", name)?),
                _ => None,
            },
        })
    }
}

struct Reader<'a> {
    decimal_sep: char,
    thousands_sep: Option<char>,
    minor_digits: u32,
    date_format: &'a str,
}

impl Reader<'_> {
    fn line<'c>(
        &self,
        layout: &Layout,
        line: usize,
        cell: impl Fn(usize) -> &'c str,
    ) -> ParsedLine {
        let mut errors = Vec::new();
        let date = parse_date(cell(layout.date), self.date_format)
            .map_err(|e| errors.push(e))
            .ok();
        let amount_minor = self.amount(layout, &cell).map_err(|e| errors.push(e)).ok();
        let balance_after_minor = layout.balance.and_then(|index| {
            let text = cell(index);
            if text.is_empty() {
                return None;
            }
            self.money(text)
                .map_err(|e| errors.push(format!("Balance: {e}")))
                .ok()
        });
        let description = layout
            .description
            .iter()
            .map(|&index| cell(index))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let description = description.split_whitespace().collect::<Vec<_>>().join(" ");
        ParsedLine {
            line,
            date,
            amount_minor,
            description: if description.is_empty() {
                "(no description)".to_owned()
            } else {
                description
            },
            balance_after_minor,
            error: (!errors.is_empty()).then(|| errors.join(" ")),
        }
    }

    fn amount<'c>(&self, layout: &Layout, cell: &impl Fn(usize) -> &'c str) -> Result<i64, String> {
        let amount = match layout.amount {
            AmountColumns::Signed(index) => {
                let text = cell(index);
                if text.is_empty() {
                    return Err("There is no amount.".to_owned());
                }
                self.money(text)?
            }
            AmountColumns::Split { debit, credit } => {
                let (debit, credit) = (cell(debit), cell(credit));
                if debit.is_empty() && credit.is_empty() {
                    return Err("There is no debit or credit amount.".to_owned());
                }
                let part = |text: &str| -> Result<i64, String> {
                    if text.is_empty() {
                        Ok(0)
                    } else {
                        self.money(text).map(i64::abs)
                    }
                };
                // Money out is negative whether the bank writes debits with a minus or not.
                part(credit)? - part(debit)?
            }
        };
        if amount == 0 {
            return Err("The amount is zero.".to_owned());
        }
        Ok(amount)
    }

    fn money(&self, text: &str) -> Result<i64, String> {
        parse_money(
            text,
            self.decimal_sep,
            self.thousands_sep,
            self.minor_digits,
        )
    }
}

fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// Parse an amount as the bank writes it into signed minor units, e.g.
/// "-1.250,50" with a decimal comma and `.` thousands -> -125050.
/// Accepts a leading or trailing sign, brackets for negatives and a currency
/// symbol.
pub(crate) fn parse_money(
    text: &str,
    decimal_sep: char,
    thousands_sep: Option<char>,
    minor_digits: u32,
) -> Result<i64, String> {
    let bad = || format!("\"{text}\" is not an amount.");
    let mut body: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '€' | '$' | '£' | '\u{a0}' | '\u{202f}'))
        .collect();
    let mut negative = false;
    if body.starts_with('(') && body.ends_with(')') && body.len() > 2 {
        negative = true;
        body = body[1..body.len() - 1].to_owned();
    }
    if let Some(rest) = body.strip_prefix('-').or_else(|| body.strip_suffix('-')) {
        negative = !negative;
        body = rest.to_owned();
    } else if let Some(rest) = body.strip_prefix('+') {
        body = rest.to_owned();
    }
    let (whole, fraction) = match body.split_once(decimal_sep) {
        Some((whole, fraction)) => (whole, fraction),
        None => (body.as_str(), ""),
    };
    // Thousands separators must group the whole part in threes: "1.250", not "12.50".
    let grouped;
    let whole = match thousands_sep.filter(|&t| t != decimal_sep && !t.is_whitespace()) {
        Some(t) if whole.contains(t) => {
            let groups: Vec<&str> = whole.split(t).collect();
            let well_grouped =
                (1..=3).contains(&groups[0].len()) && groups[1..].iter().all(|g| g.len() == 3);
            if !well_grouped {
                return Err(bad());
            }
            grouped = groups.concat();
            grouped.as_str()
        }
        _ => whole,
    };
    if (whole.is_empty() && fraction.is_empty())
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(bad());
    }
    if fraction.len() > minor_digits as usize {
        return Err(format!("\"{text}\" has more than {minor_digits} decimals."));
    }
    let digits = format!(
        "{}{:0<width$}",
        if whole.is_empty() { "0" } else { whole },
        fraction,
        width = minor_digits as usize
    );
    let units: i64 = digits.parse().map_err(|_| bad())?;
    Ok(if negative { -units } else { units })
}

/// Parse a date with a strftime-style format: `%d` day, `%m` month, `%Y`
/// four-digit year, `%y` two-digit year (20xx), and `%H`, `%M`, `%S` for a
/// time that is ignored. Any other character must match itself. Returns
/// `YYYY-MM-DD`.
pub(crate) fn parse_date(text: &str, format: &str) -> Result<String, String> {
    let bad = || format!("\"{text}\" doesn't match the date format {format}.");
    let input = text.as_bytes();
    let mut pos = 0;
    let (mut year, mut month, mut day) = (None, None, None);
    let take_number = |pos: &mut usize, min: usize, max: usize| -> Option<u32> {
        let start = *pos;
        while *pos < input.len() && *pos - start < max && input[*pos].is_ascii_digit() {
            *pos += 1;
        }
        if *pos - start < min {
            return None;
        }
        text[start..*pos].parse().ok()
    };
    let mut spec = format.chars();
    while let Some(c) = spec.next() {
        if c == '%' {
            match spec.next() {
                Some('d') => day = Some(take_number(&mut pos, 1, 2).ok_or_else(bad)?),
                Some('m') => month = Some(take_number(&mut pos, 1, 2).ok_or_else(bad)?),
                Some('Y') => year = Some(take_number(&mut pos, 4, 4).ok_or_else(bad)?),
                Some('y') => year = Some(2000 + take_number(&mut pos, 2, 2).ok_or_else(bad)?),
                Some('H' | 'M' | 'S') => {
                    take_number(&mut pos, 1, 2).ok_or_else(bad)?;
                }
                Some('%') => {
                    if input.get(pos) != Some(&b'%') {
                        return Err(bad());
                    }
                    pos += 1;
                }
                _ => return Err(format!("The date format {format} isn't supported.")),
            }
        } else {
            let mut buf = [0; 4];
            let literal = c.encode_utf8(&mut buf).as_bytes();
            if !input[pos..].starts_with(literal) {
                return Err(bad());
            }
            pos += literal.len();
        }
    }
    if pos != input.len() {
        return Err(bad());
    }
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return Err(format!(
            "The date format {format} needs a day (%d), a month (%m) and a year (%Y or %y)."
        ));
    };
    let iso = format!("{year:04}-{month:02}-{day:02}");
    if date::is_valid(&iso) {
        Ok(iso)
    } else {
        Err(bad())
    }
}

/// Check that a date format is one [`parse_date`] understands and that it
/// has a day, a month and a year.
pub(crate) fn check_date_format(format: &str) -> Result<(), String> {
    let (mut day, mut month, mut year) = (false, false, false);
    let mut spec = format.chars();
    while let Some(c) = spec.next() {
        if c == '%' {
            match spec.next() {
                Some('d') => day = true,
                Some('m') => month = true,
                Some('Y' | 'y') => year = true,
                Some('H' | 'M' | 'S' | '%') => {}
                _ => return Err(format!("The date format {format} isn't supported.")),
            }
        }
    }
    if day && month && year {
        Ok(())
    } else {
        Err(format!(
            "The date format {format} needs a day (%d), a month (%m) and a year (%Y or %y)."
        ))
    }
}
