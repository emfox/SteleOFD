//! Localized formatting of numbers, dates, file sizes and percentages.
//! Never assemble these by hand: date order, digit grouping and the decimal mark
//! all vary by language, so the rules live here.

use crate::locale::Locale;

/// How to interpret file size units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SizeUnitSystem {
    /// Binary, 1 KB is 1024 bytes.
    #[default]
    Binary,
    /// Decimal, 1 kB is 1000 bytes.
    Decimal,
}

/// Locale-aware formatter.
#[derive(Debug, Clone)]
pub struct LocalizedFormat {
    locale: Locale,
    size_system: SizeUnitSystem,
}

impl LocalizedFormat {
    /// Create a formatter for a locale.
    pub fn new(locale: Locale) -> Self {
        Self {
            locale,
            size_system: SizeUnitSystem::Binary,
        }
    }

    /// Choose the unit system for file sizes.
    pub fn with_size_system(mut self, s: SizeUnitSystem) -> Self {
        self.size_system = s;
        self
    }

    /// The locale in use.
    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    /// Format an integer with grouping separators.
    pub fn integer(&self, n: i64) -> String {
        group_digits(n, separator_for(self.locale.tag()))
    }

    /// Format a decimal with the locale decimal mark.
    pub fn decimal(&self, v: f64, fraction_digits: usize) -> String {
        let s = format!("{v:.fraction_digits$}");
        let (int_part, frac_part) = s.split_once('.').unwrap_or((s.as_str(), ""));
        let int_grouped = group_digits(
            int_part.parse::<i64>().unwrap_or(0),
            separator_for(self.locale.tag()),
        );
        let dot = decimal_point_for(self.locale.tag());
        if fraction_digits == 0 {
            int_grouped
        } else {
            format!("{int_grouped}{dot}{frac_part}")
        }
    }

    /// Format a ratio as a percentage.
    pub fn percent(&self, ratio: f64, fraction_digits: usize) -> String {
        format!(
            "{}{}",
            self.decimal(ratio * 100.0, fraction_digits),
            percent_sign_for(self.locale.tag())
        )
    }

    /// Format a byte count with an automatically chosen unit.
    pub fn file_size(&self, bytes: u64) -> String {
        let (base, units) = match self.size_system {
            SizeUnitSystem::Binary => (1024.0, ["B", "KB", "MB", "GB", "TB"]),
            SizeUnitSystem::Decimal => (1000.0, ["B", "kB", "MB", "GB", "TB"]),
        };

        let mut size = bytes as f64;
        let mut idx = 0;
        while size >= base && idx < units.len() - 1 {
            size /= base;
            idx += 1;
        }

        if idx == 0 {
            format!("{} {}", self.integer(bytes as i64), units[idx])
        } else if size < 10.0 {
            format!("{} {}", self.decimal(size, 1), units[idx])
        } else {
            format!("{} {}", self.integer(size.round() as i64), units[idx])
        }
    }

    /// Format a Unix timestamp as date and time in the locale's style.
    pub fn datetime(&self, unix_secs: i64) -> String {
        let (y, m, d, hh, mm) = civil_from_unix(unix_secs);

        match self.locale.language() {
            "zh" => {
                let (h12, ampm) = to_hour12(hh, "上午", "下午");
                format!("{y}\u{5e74}{m}\u{6708}{d}\u{65e5} {ampm}{h12}:{mm:02}")
            }
            "ja" => {
                let (h12, ampm) = to_hour12(hh, "午前", "午後");
                format!("{y}\u{5e74}{m}\u{6708}{d}\u{65e5} {ampm}{h12}:{mm:02}")
            }
            _ => {
                let mon = month_abbr(m);
                let (h12, ampm) = match hh {
                    0 => (12, "AM"),
                    h if h < 12 => (h, "AM"),
                    12 => (12, "PM"),
                    h => (h - 12, "PM"),
                };
                format!("{mon} {d}, {y}, {h12}:{mm:02} {ampm}")
            }
        }
    }

    /// Format a Unix timestamp as a date with no time.
    pub fn date(&self, unix_secs: i64) -> String {
        let (y, m, d, _, _) = civil_from_unix(unix_secs);
        match self.locale.language() {
            "zh" => format!("{y}\u{5e74}{m}\u{6708}{d}\u{65e5}"),
            "ja" => format!("{y}\u{5e74}{m}\u{6708}{d}\u{65e5}"),
            _ => format!("{} {d}, {y}", month_abbr(m)),
        }
    }

    /// Describe a page size, for example `210 x 297 mm`.
    pub fn page_size(&self, w_mm: f64, h_mm: f64) -> String {
        let dot = decimal_point_for(self.locale.tag());
        let fmt = |v: f64| {
            let s = format!("{v:.1}");
            if dot == '.' {
                s
            } else {
                s.replace('.', &dot.to_string())
            }
        };
        format!("{} \u{00d7} {} mm", fmt(w_mm), fmt(h_mm))
    }
}

/// Twelve-hour clock with an AM/PM label in the given language.
fn to_hour12<'a>(hh: u32, am: &'a str, pm: &'a str) -> (u32, &'a str) {
    match hh {
        0 => (12, am),
        h if h < 12 => (h, am),
        12 => (12, pm),
        h => (h - 12, pm),
    }
}

/// English month abbreviation, falling back to January.
fn month_abbr(m: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    MONTHS
        .get((m as usize).saturating_sub(1))
        .copied()
        .unwrap_or("Jan")
}

/// Digit grouping separator for a language.
fn separator_for(tag: &str) -> char {
    match tag.split('-').next().unwrap_or(tag) {
        "fr" | "ru" | "sv" | "pl" | "cs" | "uk" => '\u{202F}',
        "de" | "it" | "es" | "pt" | "nl" | "da" | "tr" => '.',
        _ => ',',
    }
}

/// Decimal point for a language.
fn decimal_point_for(tag: &str) -> char {
    match tag.split('-').next().unwrap_or(tag) {
        "de" | "it" | "es" | "fr" | "pt" | "ru" | "nl" | "tr" => ',',
        _ => '.',
    }
}

/// Percent symbol; the caller decides whether it leads or trails.
fn percent_sign_for(tag: &str) -> &'static str {
    let _ = tag;
    "%"
}

/// Insert grouping separators every three digits.
fn group_digits(n: i64, sep: char) -> String {
    let neg = n < 0;
    let s = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3 + 1);

    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(ch);
    }

    if neg { format!("-{out}") } else { out }
}

/// Convert a Unix timestamp to a civil date using Howard Hinnant's algorithm.
/// Returns `(year, month, day, hour, minute)` in UTC; callers needing local time
/// should shift the timestamp first.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hh = (rem / 3600) as u32;
    let mm = ((rem % 3600) / 60) as u32;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };

    (y, m, d, hh, mm)
}
