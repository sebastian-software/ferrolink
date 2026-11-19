//! Validate the calendar date and optional zoned time of an XML sitemap lastmod.

pub(super) fn valid(value: &str) -> bool {
    let (date, timestamp) = value
        .split_once('T')
        .map_or((value, None), |(date, time)| (date, Some(time)));
    valid_date(date.as_bytes()) && timestamp.is_none_or(valid_timestamp)
}

fn valid_date(date: &[u8]) -> bool {
    if date.len() != 10 || date[4] != b'-' || date[7] != b'-' {
        return false;
    }
    let (Some(year), Some(month), Some(day)) = (
        decimal(&date[..4]),
        decimal(&date[5..7]),
        decimal(&date[8..]),
    ) else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        2 => 28 + u16::from(leap),
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    year != 0 && (1..=days).contains(&day)
}

fn valid_timestamp(timestamp: &str) -> bool {
    let Some(zone_start) = timestamp.find(['Z', '+', '-']) else {
        return false;
    };
    let (clock, zone) = timestamp.split_at(zone_start);
    let valid_zone = match zone.as_bytes() {
        [b'Z'] => true,
        [b'+' | b'-', h1, h2, b':', m1, m2] => decimal(&[*h1, *h2]).is_some_and(|hour| {
            decimal(&[*m1, *m2])
                .is_some_and(|minute| (hour < 14 && minute < 60) || (hour == 14 && minute == 0))
        }),
        _ => false,
    };
    valid_zone && valid_clock(clock.as_bytes())
}

fn valid_clock(clock: &[u8]) -> bool {
    let Some(([h1, h2, b':', m1, m2], tail)) = clock.split_first_chunk::<5>() else {
        return false;
    };
    if !decimal(&[*h1, *h2]).is_some_and(|hour| hour < 24)
        || !decimal(&[*m1, *m2]).is_some_and(|minute| minute < 60)
    {
        return false;
    }
    match tail {
        [] => true,
        [b':', s1, s2, fraction @ ..] => {
            decimal(&[*s1, *s2]).is_some_and(|second| second < 60)
                && match fraction {
                    [] => true,
                    [b'.', digits @ ..] => {
                        !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
                    }
                    _ => false,
                }
        }
        _ => false,
    }
}

// Callers pass fields of two or four bytes.
fn decimal(bytes: &[u8]) -> Option<u16> {
    bytes.iter().try_fold(0, |value, byte| {
        byte.is_ascii_digit()
            .then(|| value * 10 + u16::from(byte - b'0'))
    })
}

#[cfg(test)]
mod tests {
    use super::valid;

    #[test]
    fn validates_calendar_dates_and_zoned_clock_times() {
        for value in [
            "2024-02-29",
            "2000-02-29",
            "2025-12-31T23:59Z",
            "2025-12-31T23:59:59.006-08:30",
            "2025-12-31T00:01:00+14:00",
        ] {
            assert!(valid(value), "{value}");
        }
        for value in [
            "",
            "2025",
            "2025-03",
            "2025-02-29",
            "1900-02-29",
            "2024-04-31",
            "2024-01-00",
            "2024-13-01",
            "2024-01-01T24:01Z",
            "2024-01-01T12:60Z",
            "2024-01-01T12:00:60Z",
            "2024-01-01T12:00:00.Z",
            "2024-01-01T12:00:00",
            "2024-01-01T12:00:00+14:01",
            "2024-01-01T12:00:00+01:60",
            "2024-01-01T12:00:00Zjunk",
        ] {
            assert!(!valid(value), "{value}");
        }
    }
}
