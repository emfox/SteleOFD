//! Wall-clock values written as text, as the standard and the date types it
//! borrows from write them. A document states a moment in several notations and
//! a comparison needs one, so everything here answers Unix seconds.

/// Seconds in a day, and in the 400-year cycle the leap rule repeats on.
const DAY: i64 = 86_400;
const CYCLE_DAYS: i64 = 146_097;

/// Parse `YYYY-MM-DD` and an optional clock into Unix seconds. A value with no
/// offset is UTC, as the standard's attributes mean, rather than a guess at
/// which zone the producer sat in.
pub fn parse_utc(text: &str) -> Option<i64> {
    let text = text.trim();
    let (date, rest) = match text.len() {
        10 => (text, ""),
        _ if text.len() > 10 => (&text[..10], &text[10..]),
        _ => return None,
    };
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let rest = rest.trim_start_matches(['T', 't', ' ']);
    let (clock, offset) = split_offset(rest);
    let (hour, minute, second) = match clock {
        "" => (0, 0, 0),
        _ => {
            let mut fields = clock.split(':');
            let hour: i64 = fields.next()?.parse().ok()?;
            let minute: i64 = fields.next()?.parse().ok()?;
            let second: i64 = match fields.next() {
                // A fraction is finer than a validity window ever is.
                Some(s) => s.split('.').next()?.parse().ok()?,
                None => 0,
            };
            if fields.next().is_some() || hour > 23 || minute > 59 || second > 60 {
                return None;
            }
            (hour, minute, second)
        }
    };
    let days = days_from_civil(year, month, day)?;
    Some(days * DAY + hour * 3600 + minute * 60 + second - offset)
}

/// Parse a `ST_DateTime` written as `YYYYMMDDhhmmss` with an optional `Z`, the
/// notation GB/T 20520 uses for a timestamp's generation time.
pub fn parse_compact_utc(text: &str) -> Option<i64> {
    let text = text.trim().trim_end_matches('Z');
    if text.len() != 14 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let field = |from: usize| text[from..from + 2].parse::<i64>().ok();
    let year: i64 = text[..4].parse().ok()?;
    let days = days_from_civil(year, field(4)?, field(6)?)?;
    Some(days * DAY + field(8)? * 3600 + field(10)? * 60 + field(12)?)
}

/// Split a clock from a trailing offset, returning the clock and the offset in
/// seconds east of UTC.
fn split_offset(rest: &str) -> (&str, i64) {
    if let Some(clock) = rest.strip_suffix(['Z', 'z']) {
        return (clock, 0);
    }
    // An offset starts at the last sign that is not the leading one of a date.
    let cut = rest
        .char_indices()
        .rev()
        .find(|(at, ch)| *ch == '+' || (*ch == '-' && *at > 0))
        .map(|(at, _)| at);
    let Some(at) = cut else {
        return (rest, 0);
    };
    let (clock, zone) = rest.split_at(at);
    let seconds = match zone.len() {
        6 => {
            let hours: i64 = zone[1..3].parse().unwrap_or(0);
            let minutes: i64 = zone[4..6].parse().unwrap_or(0);
            (hours * 3600 + minutes * 60) * if zone.starts_with('-') { -1 } else { 1 }
        }
        _ => 0,
    };
    (clock, seconds)
}

/// Days from the Unix epoch to a proleptic Gregorian date, in the branch-free
/// form Howard Hinnant published: it makes `1970-01-01` zero without a table.
fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    let month_days = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let longest = if month == 2 && !leap {
        28
    } else {
        month_days[month as usize - 1]
    };
    if day > longest {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted = (month + 9) % 12;
    let day_of_year = (153 * shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * CYCLE_DAYS + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_is_zero_in_every_notation() {
        assert_eq!(parse_utc("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc("1970-01-01"), Some(0));
        assert_eq!(parse_compact_utc("19700101000000Z"), Some(0));
    }

    #[test]
    fn a_date_utc_and_the_same_date_with_a_zone_are_one_moment() {
        let utc = parse_utc("2024-05-01T09:00:00Z");
        assert_eq!(utc, parse_utc("2024-05-01 17:00:00+08:00"));
        assert_eq!(utc, parse_utc("2024-05-01T09:00:00"));
    }

    #[test]
    fn a_leap_day_counts_and_a_day_that_is_not_there_does_not() {
        assert!(parse_utc("2024-02-29T00:00:00Z").is_some());
        assert_eq!(parse_utc("2023-02-29T00:00:00Z"), None);
        assert_eq!(parse_utc("2024-13-01T00:00:00Z"), None);
        assert_eq!(parse_utc("not a date"), None);
    }

    #[test]
    fn a_fraction_is_read_and_does_not_change_the_second() {
        assert_eq!(
            parse_utc("2024-05-01T09:00:00.250Z"),
            parse_utc("2024-05-01T09:00:00Z")
        );
    }

    #[test]
    fn the_compact_form_matches_the_long_one() {
        assert_eq!(
            parse_compact_utc("20260101000000Z"),
            parse_utc("2026-01-01T00:00:00Z")
        );
        assert_eq!(parse_compact_utc("20260101"), None);
    }
}
