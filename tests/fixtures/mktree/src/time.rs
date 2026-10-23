//! Instants, ages and their text forms, in integer arithmetic only so that
//! every platform computes the same values.

use crate::{Error, Result};

/// Seconds in the units an age accepts. A year is 365 days, as in duscope's
/// own time grammar.
const UNITS: &[(char, i64)] = &[
    ('s', 1),
    ('h', 3_600),
    ('d', 86_400),
    ('w', 604_800),
    ('y', 31_536_000),
];

/// A point in time: whole seconds since the Unix epoch (negative before
/// 1970) and the nanoseconds within that second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    /// Seconds since 1970-01-01T00:00:00Z.
    pub secs: i64,
    /// Nanoseconds, below 1,000,000,000.
    pub nanos: u32,
}

impl Stamp {
    /// A stamp on a whole second.
    pub fn secs(secs: i64) -> Stamp {
        Stamp { secs, nanos: 0 }
    }

    /// `secs.nanos` with nine fractional digits, the form `find -printf %T@`
    /// prints and the expanded manifest uses.
    pub fn text(self) -> String {
        format!("{}.{:09}", self.secs, self.nanos)
    }

    /// The stamp as a [`std::time::SystemTime`].
    pub fn system_time(self) -> std::time::SystemTime {
        use std::time::{Duration, UNIX_EPOCH};
        let base = Duration::from_secs(self.secs.unsigned_abs());
        let t = if self.secs >= 0 {
            UNIX_EPOCH + base
        } else {
            UNIX_EPOCH - base
        };
        t + Duration::from_nanos(u64::from(self.nanos))
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * mp + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The date of a day count since 1970-01-01, as (year, month, day).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

/// Parses an RFC 3339 instant of the one shape the manifest allows,
/// `YYYY-MM-DDTHH:MM:SSZ`, into seconds since the epoch.
pub fn parse_instant(text: &str) -> Result<i64> {
    let bad = || {
        Error::new(format!(
            "instant {text:?} is not of the form YYYY-MM-DDTHH:MM:SSZ"
        ))
    };
    let b = text.as_bytes();
    let shape = b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        });
    if !shape {
        return Err(bad());
    }
    let num = |range: std::ops::Range<usize>| -> i64 {
        b[range].iter().fold(0, |n, c| n * 10 + i64::from(c - b'0'))
    };
    let (year, month, day) = (num(0..4), num(5..7), num(8..10));
    let (hour, minute, second) = (num(11..13), num(14..16), num(17..19));
    let valid = (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60;
    if !valid {
        return Err(bad());
    }
    Ok(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Formats seconds since the epoch as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_instant(secs: i64) -> String {
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

/// The `[[CC]YY]MMDDhhmm.SS` argument of `touch -t` for a UTC instant; the
/// caller runs touch with `TZ=UTC`.
pub fn touch_stamp(secs: i64) -> String {
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{year:04}{month:02}{day:02}{:02}{:02}.{:02}",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

/// Parses an age such as `30d`, `6h`, `2y` or `-1d` (in the future) into
/// seconds. Units: `s`, `h`, `d`, `w` and `y` (365 days); `m` is refused as
/// ambiguous, as duscope's own time grammar refuses it.
pub fn parse_age(text: &str) -> Result<i64> {
    let bad = || Error::new(format!("age {text:?} is not <integer><s|h|d|w|y>"));
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let unit = body.chars().last().ok_or_else(bad)?;
    let &(_, scale) = UNITS.iter().find(|(u, _)| *u == unit).ok_or_else(bad)?;
    let digits = &body[..body.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    let n: i64 = digits.parse().map_err(|_| bad())?;
    let secs = n.checked_mul(scale).ok_or_else(bad)?;
    Ok(if negative { -secs } else { secs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_test_01_instants_round_trip() {
        for text in [
            "1970-01-01T00:00:00Z",
            "1969-07-20T20:17:40Z",
            "2000-02-29T23:59:59Z",
            "2030-01-01T00:00:00Z",
            "2037-12-31T23:59:59Z",
        ] {
            assert_eq!(format_instant(parse_instant(text).unwrap()), text);
        }
        assert_eq!(parse_instant("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(
            parse_instant("2030-01-01T00:00:00Z").unwrap(),
            1_893_456_000
        );
        assert_eq!(parse_instant("1969-12-31T23:59:59Z").unwrap(), -1);
    }

    #[test]
    fn p0_test_01_instants_reject_other_shapes() {
        for text in [
            "2030-01-01T00:00:00",
            "2030-01-01 00:00:00Z",
            "2030-02-30T00:00:00Z",
            "2030-01-01T24:00:00Z",
            "2030-01-01T00:00:00.5Z",
            "2030-01-01T00:00:00+00:00",
        ] {
            assert!(parse_instant(text).is_err(), "{text}");
        }
    }

    #[test]
    fn p0_test_01_touch_stamps_are_utc() {
        assert_eq!(touch_stamp(0), "197001010000.00");
        assert_eq!(touch_stamp(-14_182_940), "196907202017.40");
        assert_eq!(touch_stamp(1_893_455_999), "202912312359.59");
    }

    #[test]
    fn p0_test_01_ages_parse() {
        assert_eq!(parse_age("45s").unwrap(), 45);
        assert_eq!(parse_age("6h").unwrap(), 21_600);
        assert_eq!(parse_age("30d").unwrap(), 2_592_000);
        assert_eq!(parse_age("2w").unwrap(), 1_209_600);
        assert_eq!(parse_age("1y").unwrap(), 31_536_000);
        assert_eq!(parse_age("-1d").unwrap(), -86_400);
        assert_eq!(parse_age("0s").unwrap(), 0);
        for bad in ["", "d", "5m", "1.5d", "+1d", "1 d", "--1d"] {
            assert!(parse_age(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn p0_test_01_stamps_print_nine_digits() {
        assert_eq!(Stamp { secs: -5, nanos: 7 }.text(), "-5.000000007");
        assert_eq!(Stamp::secs(1_893_456_000).text(), "1893456000.000000000");
    }
}
