//! Which backups to delete, by Spatie's default strategy: keep everything newer than `keep_all_days`; then
//! the newest backup of each day, week, month and year within their ranges; delete the rest. The newest
//! backup is never deleted.

use chrono::{DateTime, Datelike, Duration, Months, Utc};

use crate::config::Retention;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    /// Path on the disk, for example `my-app/2026-10-30-01-30-00.sql.gz`.
    pub path: String,
    pub size: u64,
    pub taken_at: DateTime<Utc>,
}

/// A range of time and how to tell which period (day, week, month, year) a moment belongs to.
type Period = (
    DateTime<Utc>,
    DateTime<Utc>,
    fn(&DateTime<Utc>) -> (i32, u32, u32),
);

/// Indexes (into `backups`) of the backups that retention says to delete.
pub fn to_delete(backups: &[Backup], retention: &Retention, now: DateTime<Utc>) -> Vec<usize> {
    let mut order: Vec<usize> = (0..backups.len()).collect();
    order.sort_by_key(|&index| std::cmp::Reverse(backups[index].taken_at));
    let Some(&newest) = order.first() else {
        return Vec::new();
    };

    let all_start = now - Duration::days(retention.keep_all_days);
    let daily_start = all_start - Duration::days(retention.keep_daily_days);
    let weekly_start = daily_start - Duration::weeks(retention.keep_weekly_weeks);
    let monthly_start = weekly_start
        .checked_sub_months(Months::new(retention.keep_monthly_months.max(0) as u32))
        .unwrap_or(weekly_start);
    let yearly_start = monthly_start
        .checked_sub_months(Months::new(
            (retention.keep_yearly_years.max(0) * 12) as u32,
        ))
        .unwrap_or(monthly_start);

    let mut keep = vec![false; backups.len()];
    keep[newest] = true;
    for &index in &order {
        if backups[index].taken_at >= all_start {
            keep[index] = true;
        }
    }

    // Newest first, so the first backup seen for a period is the one that stays.
    let periods: [Period; 4] = [
        (daily_start, all_start, |t| (t.year(), t.month(), t.day())),
        (weekly_start, daily_start, |t| {
            (t.iso_week().year(), t.iso_week().week(), 0)
        }),
        (monthly_start, weekly_start, |t| (t.year(), t.month(), 0)),
        (yearly_start, monthly_start, |t| (t.year(), 0, 0)),
    ];
    for (start, end, key) in periods {
        let mut seen = std::collections::HashSet::new();
        for &index in &order {
            let at = backups[index].taken_at;
            if at >= start && at < end && seen.insert(key(&at)) {
                keep[index] = true;
            }
        }
    }

    (0..backups.len()).filter(|&index| !keep[index]).collect()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(days_ago: i64, now: DateTime<Utc>) -> Backup {
        let taken_at = now - Duration::days(days_ago);
        Backup {
            path: format!("app/{}.sql.gz", taken_at.format("%Y-%m-%d-%H-%M-%S")),
            size: 1,
            taken_at,
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 30, 12, 0, 0).unwrap()
    }

    fn deleted_days(backups: &[Backup], retention: &Retention) -> Vec<i64> {
        to_delete(backups, retention, now())
            .into_iter()
            .map(|index| (now() - backups[index].taken_at).num_days())
            .collect()
    }

    #[test]
    fn everything_recent_is_kept() {
        let backups: Vec<_> = (0..7).map(|d| at(d, now())).collect();
        assert!(to_delete(&backups, &Retention::default(), now()).is_empty());
        assert!(to_delete(&[], &Retention::default(), now()).is_empty());
    }

    #[test]
    fn the_newest_backup_is_never_deleted_even_when_it_is_ancient() {
        let backups = vec![at(900, now()), at(1000, now())];
        assert_eq!(deleted_days(&backups, &Retention::default()), vec![1000]);
    }

    #[test]
    fn one_backup_per_day_is_kept_in_the_daily_window() {
        let retention = Retention {
            keep_all_days: 2,
            keep_daily_days: 5,
            keep_weekly_weeks: 0,
            keep_monthly_months: 0,
            keep_yearly_years: 0,
        };
        let mut backups = vec![at(1, now()), at(0, now())];
        // Day 4 (inside the daily window) has three backups taken at different hours.
        for hours in [0, 5, 10] {
            let mut b = at(4, now());
            b.taken_at = b.taken_at + Duration::hours(hours) - Duration::hours(6);
            backups.push(b);
        }
        let deleted = to_delete(&backups, &retention, now());
        let kept: Vec<_> = (0..backups.len())
            .filter(|i| !deleted.contains(i))
            .collect();
        assert_eq!(deleted.len(), 2, "two of the three same-day backups go");
        // The newest of that day (index 4, +4 hours) is the one that stays.
        assert!(kept.contains(&4) && !kept.contains(&2) && !kept.contains(&3));
    }

    #[test]
    fn old_backups_thin_out_to_weekly_monthly_and_yearly_then_disappear() {
        let retention = Retention {
            keep_all_days: 1,
            keep_daily_days: 1,
            keep_weekly_weeks: 2,
            keep_monthly_months: 2,
            keep_yearly_years: 1,
        };
        let backups: Vec<_> = (0..800).step_by(3).map(|d| at(d, now())).collect();
        let deleted = to_delete(&backups, &retention, now());
        let kept = backups.len() - deleted.len();
        assert!(kept < backups.len() / 4, "kept {kept} of {}", backups.len());
        let oldest_kept = (0..backups.len())
            .filter(|i| !deleted.contains(i))
            .map(|i| (now() - backups[i].taken_at).num_days())
            .max()
            .unwrap();
        assert!(
            oldest_kept < 480,
            "nothing older than the yearly range stays (oldest kept: {oldest_kept} days)"
        );
    }
}
