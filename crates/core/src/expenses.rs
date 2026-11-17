//! Pure expense aggregation for the Despesas tab (plan.md §7, T2-1).
//!
//! No I/O: the service layer fetches `Expense` rows; these functions turn
//! the slice into display-ready summaries. `now` is a parameter, never
//! read from the clock — deterministic, testable, pt-BR UI decides when to
//! call this (rules.md §4).
//!
//! A "month" is a `(year, month)` pair — chrono 0.4.45 removed
//! `YearMonth`, and plain tuples keep the keys trivially sortable.

use chrono::{Datelike, NaiveDate};

use crate::entities::Expense;

/// A month as `(year, month)`, sortable.
pub type Month = (i32, u32);

/// The month before `(y, m)`, rolling back over the year boundary.
fn previous_month(y: i32, m: u32) -> Month {
    if m == 1 {
        (y - 1, 12)
    } else {
        (y, m - 1)
    }
}

/// Aggregated current-month total plus its month-over-month delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthSummary {
    pub current_month: Month,
    pub current_total_cents: i64,
    pub previous_month: Month,
    pub previous_total_cents: i64,
    /// Signed percentage change of the current month vs. the previous one;
    /// `None` when the previous month had no spend (a delta against zero
    /// is meaningless — the UI shows a hint instead).
    pub delta_pct: Option<i32>,
}

/// Summarizes all non-deleted expenses for the month containing `now` and
/// the month before it.
pub fn month_summary(expenses: &[Expense], now: NaiveDate) -> MonthSummary {
    let current = (now.year(), now.month());
    let previous = previous_month(current.0, current.1);
    let (mut cur, mut prev) = (0_i64, 0_i64);
    for e in expenses {
        if e.deleted_at.is_some() {
            continue;
        }
        let m = (e.occurred_on.year(), e.occurred_on.month());
        match m {
            key if key == current => cur += e.amount_cents,
            key if key == previous => prev += e.amount_cents,
            _ => {}
        }
    }
    let delta_pct = if prev > 0 {
        Some(((cur - prev) * 100 / prev) as i32)
    } else {
        None
    };
    MonthSummary {
        current_month: current,
        current_total_cents: cur,
        previous_month: previous,
        previous_total_cents: prev,
        delta_pct,
    }
}

/// Groups non-deleted expenses by month, newest month first; within a
/// month, newest date first.
pub fn group_by_month(expenses: &[Expense]) -> Vec<(Month, Vec<Expense>)> {
    let mut by_month: std::collections::BTreeMap<Month, Vec<Expense>> =
        std::collections::BTreeMap::new();
    for e in expenses {
        if e.deleted_at.is_some() {
            continue;
        }
        let m = (e.occurred_on.year(), e.occurred_on.month());
        by_month.entry(m).or_default().push(e.clone());
    }
    let mut out: Vec<(Month, Vec<Expense>)> = by_month.into_iter().collect();
    out.sort_by(|a, b| a.0.cmp(&b.0).reverse());
    for (_, rows) in &mut out {
        rows.sort_by(|a, b| b.occurred_on.cmp(&a.occurred_on));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::ExpenseCategory;
    use crate::{Utc, Uuid};

    fn expense(occurred: &str, cents: i64, deleted: bool) -> Expense {
        Expense {
            id: Uuid::new_v4(),
            vehicle_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            category: ExpenseCategory::Fuel,
            title: "x".into(),
            amount_cents: cents,
            occurred_on: NaiveDate::parse_from_str(occurred, "%Y-%m-%d").unwrap(),
            odometer_km: None,
            location: None,
            notes: None,
            deleted_at: deleted.then(|| Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn empty_list_is_zero_with_no_delta() {
        let s = month_summary(&[], NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(s.current_total_cents, 0);
        assert_eq!(s.previous_total_cents, 0);
        assert_eq!(s.delta_pct, None);
    }

    #[test]
    fn sums_current_and_previous_month_only() {
        let rows = vec![
            expense("2026-11-01", 100_000, false),
            expense("2026-11-20", 50_000, false),
            expense("2026-10-31", 75_000, false),
            expense("2026-09-15", 999_000, false), // older: ignored
        ];
        let s = month_summary(&rows, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(s.current_month, (2026, 11));
        assert_eq!(s.current_total_cents, 150_000);
        assert_eq!(s.previous_total_cents, 75_000);
    }

    #[test]
    fn delta_pct_is_signed_percent_of_previous() {
        let rows = vec![
            expense("2026-11-05", 150_000, false),
            expense("2026-10-05", 100_000, false),
        ];
        let s = month_summary(&rows, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(s.delta_pct, Some(50));

        let rows = vec![
            expense("2026-11-05", 50_000, false),
            expense("2026-10-05", 100_000, false),
        ];
        let s = month_summary(&rows, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(s.delta_pct, Some(-50));

        let rows = vec![expense("2026-11-05", 100_000, false)];
        let s = month_summary(&rows, NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        assert_eq!(s.delta_pct, None, "no previous spend → no delta");
    }

    #[test]
    fn ignores_soft_deleted_rows() {
        let rows = vec![
            expense("2026-11-01", 100_000, false),
            expense("2026-11-02", 50_000, true),
        ];
        let s = month_summary(&rows, NaiveDate::from_ymd_opt(2026, 11, 10).unwrap());
        assert_eq!(s.current_total_cents, 100_000);
    }

    #[test]
    fn previous_month_rolls_back_over_the_year_boundary() {
        let rows = vec![
            expense("2027-01-10", 30_000, false),
            expense("2026-12-31", 10_000, false),
            expense("2026-11-30", 80_000, false), // two months back: ignored
        ];
        let s = month_summary(&rows, NaiveDate::from_ymd_opt(2027, 1, 15).unwrap());
        assert_eq!(s.current_month, (2027, 1));
        assert_eq!(s.previous_month, (2026, 12));
        assert_eq!(s.current_total_cents, 30_000);
        assert_eq!(s.previous_total_cents, 10_000);
    }

    #[test]
    fn group_by_month_orders_newest_month_first() {
        let rows = vec![
            expense("2026-11-10", 1, false),
            expense("2026-09-15", 2, false),
            expense("2026-10-20", 3, false),
            expense("2026-11-01", 4, true), // deleted: absent
        ];
        let g = group_by_month(&rows);
        let months: Vec<Month> = g.iter().map(|(m, _)| *m).collect();
        assert_eq!(months, vec![(2026, 11), (2026, 10), (2026, 9)]);
        assert_eq!(g[0].1.len(), 1);
    }

    #[test]
    fn group_by_month_orders_within_month_newest_first() {
        let rows = vec![
            expense("2026-11-01", 1, false),
            expense("2026-11-20", 2, false),
            expense("2026-11-10", 3, false),
        ];
        let g = group_by_month(&rows);
        assert_eq!(g.len(), 1);
        let cents: Vec<i64> = g[0].1.iter().map(|e| e.amount_cents).collect();
        assert_eq!(cents, vec![2, 3, 1]);
    }
}
