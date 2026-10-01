//! The History window's days (DLG-HIS-02): entries grouped under the day they
//! were opened ("Today", "Yesterday", "Monday", "10 September"), each row
//! showing only its time, in the user's clock format. GTK-free apart from
//! `GDateTime`, so it is tested without a display.

use gtk::prelude::*;
use gtk::{gio, glib};

/// The days a weekday name stands for, after "Yesterday".
const WEEKDAY_SPAN: i64 = 6;

/// Microseconds in a day.
const DAY_MICROSECONDS: i64 = 86_400_000_000;

/// One day of the list: its header and the indices of its rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    pub title: String,
    pub rows: Vec<usize>,
}

/// The local midnight starting the day of `time` (Unix seconds).
fn midnight(time: i64, zone: &glib::TimeZone) -> Option<glib::DateTime> {
    let moment = glib::DateTime::from_unix_utc(time)
        .ok()?
        .to_timezone(zone)
        .ok()?;
    glib::DateTime::new(
        zone,
        moment.year(),
        moment.month(),
        moment.day_of_month(),
        0,
        0,
        0.0,
    )
    .ok()
}

/// Whole days from `day` to `today`, both local midnights. Rounded, so a
/// daylight-saving change (a 23- or 25-hour day) still counts as one.
fn days_between(day: &glib::DateTime, today: &glib::DateTime) -> i64 {
    let span = today.difference(day).as_microseconds();
    (span + DAY_MICROSECONDS / 2).div_euclid(DAY_MICROSECONDS)
}

/// The header of `day` seen from `today`.
fn title(day: &glib::DateTime, today: &glib::DateTime) -> String {
    let format = match days_between(day, today) {
        0 => return "Today".to_owned(),
        1 => return "Yesterday".to_owned(),
        2..=WEEKDAY_SPAN => "%A",
        _ if day.year() == today.year() => "%A, %-d %B",
        _ => "%-d %B %Y",
    };
    day.format(format).map(Into::into).unwrap_or_default()
}

/// Group `times` (newest first, as the list shows them) into days, as seen
/// at `now`. Rows keep their order; a day never repeats.
#[must_use]
pub fn group(times: &[i64], now: i64, zone: &glib::TimeZone) -> Vec<Day> {
    let Some(today) = midnight(now, zone) else {
        return vec![Day {
            title: String::new(),
            rows: (0..times.len()).collect(),
        }];
    };
    let mut days: Vec<(glib::DateTime, Day)> = Vec::new();
    for (index, &time) in times.iter().enumerate() {
        let Some(day) = midnight(time, zone) else {
            continue;
        };
        if let Some((_, found)) = days.iter_mut().find(|(start, _)| *start == day) {
            found.rows.push(index);
        } else {
            let header = title(&day, &today);
            days.push((
                day,
                Day {
                    title: header,
                    rows: vec![index],
                },
            ));
        }
    }
    days.into_iter().map(|(_, day)| day).collect()
}

/// The time of day of `time` ("08:39", or "8:39 AM" on a 12-hour clock).
#[must_use]
pub fn time_of_day(time: i64, zone: &glib::TimeZone, twelve_hour: bool) -> String {
    let format = if twelve_hour { "%-l:%M %p" } else { "%H:%M" };
    glib::DateTime::from_unix_utc(time)
        .and_then(|moment| moment.to_timezone(zone))
        .ok()
        .and_then(|moment| moment.format(format).ok())
        .map(|text| text.trim().to_owned())
        .unwrap_or_default()
}

/// Whether the desktop shows a 12-hour clock (GNOME's `clock-format`).
/// Without GNOME's schema (another desktop, the self-test) it is 24-hour.
#[must_use]
pub fn twelve_hour_clock() -> bool {
    const SCHEMA: &str = "org.gnome.desktop.interface";
    const KEY: &str = "clock-format";
    let Some(schema) = gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup(SCHEMA, true))
        .filter(|schema| schema.has_key(KEY))
    else {
        return false;
    };
    gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None).string(KEY) == "12h"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-09-10 12:00 UTC, a Thursday.
    const NOW: i64 = 1_789_041_600;
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;

    fn utc() -> glib::TimeZone {
        glib::TimeZone::utc()
    }

    #[test]
    fn today_and_yesterday_are_named() {
        // DLG-HIS-02
        let days = group(&[NOW - HOUR, NOW - 2 * HOUR, NOW - DAY], NOW, &utc());
        let titles: Vec<&str> = days.iter().map(|day| day.title.as_str()).collect();
        assert_eq!(titles, ["Today", "Yesterday"]);
        assert_eq!(days[0].rows, [0, 1]);
        assert_eq!(days[1].rows, [2]);
    }

    #[test]
    fn the_last_week_reads_as_weekdays_and_older_days_as_dates() {
        let days = group(
            &[NOW - 3 * DAY, NOW - 20 * DAY, NOW - 400 * DAY],
            NOW,
            &utc(),
        );
        let titles: Vec<&str> = days.iter().map(|day| day.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Monday", "Friday, 21 August", "6 August 2025"],
            "{titles:?}"
        );
    }

    #[test]
    fn a_day_never_repeats_and_rows_keep_their_order() {
        let days = group(&[NOW, NOW - DAY, NOW - HOUR], NOW, &utc());
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].rows, [0, 2]);
    }

    #[test]
    fn times_follow_the_clock_format() {
        let morning = NOW - 3 * HOUR - 21 * 60;
        assert_eq!(time_of_day(morning, &utc(), false), "08:39");
        assert_eq!(time_of_day(morning, &utc(), true), "8:39 AM");
    }
}
