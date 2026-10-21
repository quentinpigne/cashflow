//! Récurrences (spec 02, R-TRX-40 à R-TRX-44) : échéances prévues, calculées à la volée et jamais
//! stockées comme transactions, et rapprochement avec les transactions réelles.

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::shared::{Date, Money};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Frequency {
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

/// Le calendrier d'une récurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub frequency: Frequency,
    /// Tous les `interval` pas (2 = tous les deux mois).
    pub interval: u32,
    /// Première occurrence.
    pub anchor: Date,
    pub end: Option<Date>,
}

impl Schedule {
    /// Date de la `n`-ième occurrence (0 = l'ancre). Pour un pas en mois, le jour de l'ancre est gardé,
    /// ramené au dernier jour du mois s'il n'existe pas (R-TRX-41 : le 31 tombe le 28 ou le 29 février).
    pub fn nth(&self, n: u32) -> Option<Date> {
        let step = self.interval.max(1).checked_mul(n)?;
        match self.frequency {
            Frequency::Weekly => self.anchor.checked_add_days(Days::new(u64::from(step) * 7)),
            Frequency::Monthly => add_months_clamped(self.anchor, step),
            Frequency::Quarterly => add_months_clamped(self.anchor, step.checked_mul(3)?),
            Frequency::Yearly => add_months_clamped(self.anchor, step.checked_mul(12)?),
        }
    }

    /// Occurrences comprises dans `[from, to]`.
    pub fn occurrences(&self, from: Date, to: Date) -> Vec<Date> {
        let mut result = Vec::new();
        let last = self.end.map_or(to, |end| end.min(to));
        for n in 0.. {
            let Some(date) = self.nth(n) else { break };
            if date > last {
                break;
            }
            if date >= from {
                result.push(date);
            }
        }
        result
    }
}

/// `date` + `months` mois, en gardant le jour d'origine quand il existe.
fn add_months_clamped(date: Date, months: u32) -> Option<Date> {
    // `checked_add_months` de chrono ramène déjà au dernier jour du mois quand le jour n'existe pas.
    date.checked_add_months(Months::new(months))
}

/// Nombre de jours d'écart admis entre une échéance et la transaction qui l'honore (R-TRX-42).
pub const MATCH_DAYS: i64 = 5;

/// Une transaction réelle candidate pour honorer une échéance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate<T> {
    pub id: T,
    pub date: Date,
    pub amount: Money,
    /// La transaction a été générée par cette récurrence (R-TRX-43) : elle l'honore en priorité.
    pub generated: bool,
}

/// L'état d'une échéance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occurrence<T> {
    /// Honorée par une transaction réelle (R-TRX-42).
    Honored { date: Date, by: T },
    /// Passée et non honorée : « prélèvement attendu non vu ».
    Missed { date: Date },
    /// À venir.
    Upcoming { date: Date },
}

impl<T> Occurrence<T> {
    pub fn date(&self) -> Date {
        match self {
            Occurrence::Honored { date, .. } | Occurrence::Missed { date } | Occurrence::Upcoming { date } => *date,
        }
    }
}

/// Rapproche des échéances et des transactions (R-TRX-42) : chaque transaction honore au plus une
/// échéance, la plus proche en date, celles générées par la récurrence d'abord. Une échéance à venir
/// peut déjà être honorée par une transaction en avance (jusqu'à 5 jours).
pub fn match_occurrences<T: Copy + PartialEq>(
    dates: &[Date],
    amount: Money,
    tolerance: Money,
    candidates: &[Candidate<T>],
    today: Date,
) -> Vec<Occurrence<T>> {
    let mut used: Vec<T> = Vec::new();
    dates
        .iter()
        .map(|&date| {
            let best = candidates
                .iter()
                .filter(|c| !used.contains(&c.id))
                .filter(|c| (c.amount - amount).abs() <= tolerance && (c.date - date).num_days().abs() <= MATCH_DAYS)
                .min_by_key(|c| (!c.generated, (c.date - date).num_days().abs()));
            match best {
                Some(c) => {
                    used.push(c.id);
                    Occurrence::Honored { date, by: c.id }
                }
                None if date < today => Occurrence::Missed { date },
                None => Occurrence::Upcoming { date },
            }
        })
        .collect()
}

/// Nombre moyen d'occurrences par mois, pour les versements programmés d'une projection (R-PRJ-4).
pub fn per_year(schedule: &Schedule) -> u32 {
    let per = match schedule.frequency {
        Frequency::Weekly => 52,
        Frequency::Monthly => 12,
        Frequency::Quarterly => 4,
        Frequency::Yearly => 1,
    };
    per / schedule.interval.max(1)
}

/// Dernier jour du mois de `date`.
pub fn end_of_month(date: Date) -> Date {
    let first = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date);
    first.checked_add_months(Months::new(1)).and_then(|d| d.pred_opt()).unwrap_or(date)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn monthly(anchor: &str) -> Schedule {
        Schedule { frequency: Frequency::Monthly, interval: 1, anchor: d(anchor), end: None }
    }

    #[test]
    fn r_trx_41_end_of_month_anchor() {
        let rule = monthly("2027-01-31");
        assert_eq!(
            rule.occurrences(d("2027-01-01"), d("2027-04-30")),
            vec![d("2027-01-31"), d("2027-02-28"), d("2027-03-31"), d("2027-04-30")]
        );
        let leap = monthly("2028-01-31");
        assert_eq!(leap.nth(1), Some(d("2028-02-29")));
    }

    #[test]
    fn frequencies_intervals_and_end() {
        let weekly = Schedule { frequency: Frequency::Weekly, interval: 2, anchor: d("2027-01-04"), end: None };
        assert_eq!(
            weekly.occurrences(d("2027-01-01"), d("2027-02-01")),
            vec![d("2027-01-04"), d("2027-01-18"), d("2027-02-01")]
        );
        let quarterly = Schedule {
            frequency: Frequency::Quarterly,
            interval: 1,
            anchor: d("2027-01-15"),
            end: Some(d("2027-08-01")),
        };
        assert_eq!(
            quarterly.occurrences(d("2027-01-01"), d("2027-12-31")),
            vec![d("2027-01-15"), d("2027-04-15"), d("2027-07-15")]
        );
        assert_eq!(per_year(&weekly), 26);
    }

    #[test]
    fn r_trx_42_occurrences_are_honored_missed_or_upcoming() {
        let dates = monthly("2027-01-05").occurrences(d("2027-01-01"), d("2027-04-30"));
        let candidates = [
            Candidate { id: 1, date: d("2027-01-06"), amount: Money::from_minor(-2_999), generated: false },
            // Hors tolérance de montant : ne compte pas.
            Candidate { id: 2, date: d("2027-02-05"), amount: Money::from_minor(-3_500), generated: false },
            Candidate { id: 3, date: d("2027-03-02"), amount: Money::from_minor(-3_050), generated: false },
        ];
        let result =
            match_occurrences(&dates, Money::from_minor(-2_999), Money::from_minor(100), &candidates, d("2027-03-20"));
        assert_eq!(result[0], Occurrence::Honored { date: d("2027-01-05"), by: 1 });
        assert_eq!(result[1], Occurrence::Missed { date: d("2027-02-05") });
        assert_eq!(result[2], Occurrence::Honored { date: d("2027-03-05"), by: 3 });
        assert_eq!(result[3], Occurrence::Upcoming { date: d("2027-04-05") });
    }

    #[test]
    fn generated_transactions_are_preferred() {
        let dates = [d("2027-01-05")];
        let candidates = [
            Candidate { id: 1, date: d("2027-01-05"), amount: Money::from_minor(-1_000), generated: false },
            Candidate { id: 2, date: d("2027-01-08"), amount: Money::from_minor(-1_000), generated: true },
        ];
        let result = match_occurrences(&dates, Money::from_minor(-1_000), Money::ZERO, &candidates, d("2027-02-01"));
        assert_eq!(result[0], Occurrence::Honored { date: d("2027-01-05"), by: 2 });
    }
}
