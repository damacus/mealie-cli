use chrono::{Datelike, Duration, NaiveDate};

use crate::{AppError, ErrorCode};

/// Resolves the deliberately small, locale-independent date language used by meal-plan commands.
///
/// `today` is supplied by the caller so this parsing stays deterministic in tests and so a
/// command can consistently use one host-local date for every argument it resolves.
pub(crate) fn parse_date_input(
    flag: &str,
    value: &str,
    today: NaiveDate,
) -> Result<NaiveDate, AppError> {
    match value {
        "today" => return Ok(today),
        "tomorrow" => {
            return today
                .checked_add_signed(Duration::days(1))
                .ok_or_else(|| date_error(flag, value));
        }
        "yesterday" => {
            return today
                .checked_sub_signed(Duration::days(1))
                .ok_or_else(|| date_error(flag, value));
        }
        _ => {}
    }

    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(date);
    }

    if let Some(date) = parse_signed_offset(value, today) {
        return Ok(date);
    }

    Err(date_error(flag, value))
}

pub(crate) fn resolve_plan_range(
    from: Option<&str>,
    to: Option<&str>,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let from = from
        .map(|value| parse_date_input("--from", value, today))
        .transpose()?;
    let to = to
        .map(|value| parse_date_input("--to", value, today))
        .transpose()?;

    let (from, to) = match (from, to) {
        (Some(from), Some(to)) => (from, to),
        (Some(from), None) => (from, sunday_of_week(from)?),
        (None, Some(to)) => (monday_of_week(to)?, to),
        (None, None) => (today, sunday_of_week(today)?),
    };

    if from > to {
        return Err(AppError::new(
            ErrorCode::InvalidArgs,
            format!("--from ({from}) must be on or before --to ({to})"),
        ));
    }

    Ok((from, to))
}

fn parse_signed_offset(value: &str, today: NaiveDate) -> Option<NaiveDate> {
    let (unit_start, unit) = value.char_indices().next_back()?;
    let number = &value[..unit_start];
    let duration = match unit {
        'd' => Duration::try_days(number.parse::<i64>().ok()?),
        'w' => Duration::try_weeks(number.parse::<i64>().ok()?),
        _ => return None,
    }?;

    if !matches!(number.as_bytes().first(), Some(b'+') | Some(b'-')) {
        return None;
    }

    today.checked_add_signed(duration)
}

fn monday_of_week(date: NaiveDate) -> Result<NaiveDate, AppError> {
    date.checked_sub_signed(Duration::days(date.weekday().num_days_from_monday().into()))
        .ok_or_else(|| week_completion_error(date))
}

fn sunday_of_week(date: NaiveDate) -> Result<NaiveDate, AppError> {
    date.checked_add_signed(Duration::days(
        (6 - date.weekday().num_days_from_monday()).into(),
    ))
    .ok_or_else(|| week_completion_error(date))
}

fn week_completion_error(date: NaiveDate) -> AppError {
    AppError::new(
        ErrorCode::InvalidArgs,
        format!("cannot complete the ISO week containing {date}"),
    )
}

fn date_error(flag: &str, value: &str) -> AppError {
    AppError::new(
        ErrorCode::InvalidArgs,
        format!(
            "{flag} must use YYYY-MM-DD, today, tomorrow, yesterday, or a signed offset such as +2d or -1w (got \"{value}\")"
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("valid test date")
    }

    #[test]
    fn parses_iso_dates_and_relative_dates_across_boundaries() {
        let today = date("2024-02-29");

        assert_eq!(
            parse_date_input("--date", "2024-12-31", today).unwrap(),
            date("2024-12-31")
        );
        assert_eq!(parse_date_input("--date", "today", today).unwrap(), today);
        assert_eq!(
            parse_date_input("--date", "tomorrow", today).unwrap(),
            date("2024-03-01")
        );
        assert_eq!(
            parse_date_input("--date", "yesterday", today).unwrap(),
            date("2024-02-28")
        );
        assert_eq!(
            parse_date_input("--date", "+1d", date("2024-12-31")).unwrap(),
            date("2025-01-01")
        );
        assert_eq!(
            parse_date_input("--date", "-1w", date("2025-01-03")).unwrap(),
            date("2024-12-27")
        );
    }

    #[test]
    fn rejects_invalid_or_locale_dependent_dates() {
        let today = date("2026-05-13");
        for value in ["2026-99-99", "next Friday", "+d", "1d", "+1m", " +1d", "é"] {
            let error = parse_date_input("--date", value, today).expect_err("invalid date input");
            assert_eq!(error.code(), ErrorCode::InvalidArgs);
            assert!(error.to_string().contains("YYYY-MM-DD"));
        }
    }

    #[test]
    fn resolves_default_and_single_boundary_ranges_using_iso_weeks() {
        let today = date("2026-01-01"); // Thursday
        assert_eq!(
            resolve_plan_range(None, None, today).unwrap(),
            (today, date("2026-01-04"))
        );
        assert_eq!(
            resolve_plan_range(Some("2025-12-31"), None, today).unwrap(),
            (date("2025-12-31"), date("2026-01-04"))
        );
        assert_eq!(
            resolve_plan_range(None, Some("2026-01-04"), today).unwrap(),
            (date("2025-12-29"), date("2026-01-04"))
        );
    }

    #[test]
    fn rejects_resolved_reversed_ranges() {
        let error = resolve_plan_range(Some("+1d"), Some("yesterday"), date("2026-05-13"))
            .expect_err("reversed range");

        assert_eq!(error.code(), ErrorCode::InvalidArgs);
        assert_eq!(
            error.to_string(),
            "--from (2026-05-14) must be on or before --to (2026-05-12)"
        );
    }

    #[test]
    fn rejects_offsets_that_overflow_chrono_durations() {
        let today = date("2026-05-13");
        for value in [
            format!("{}d", i64::MIN),
            format!("{}d", i64::MAX),
            format!("{}w", i64::MIN),
            format!("{}w", i64::MAX),
        ] {
            let error = parse_date_input("--date", &value, today)
                .expect_err("out-of-range offset should be rejected");
            assert_eq!(error.code(), ErrorCode::InvalidArgs);
        }
    }

    #[test]
    fn rejects_iso_week_completion_past_chrono_boundaries() {
        let max_error = resolve_plan_range(Some(&NaiveDate::MAX.to_string()), None, NaiveDate::MIN)
            .expect_err("MAX date cannot be completed through Sunday");
        let min_error = resolve_plan_range(None, Some(&NaiveDate::MIN.to_string()), NaiveDate::MAX)
            .expect_err("MIN date cannot be completed back through Monday");

        assert_eq!(max_error.code(), ErrorCode::InvalidArgs);
        assert_eq!(min_error.code(), ErrorCode::InvalidArgs);
    }
}
