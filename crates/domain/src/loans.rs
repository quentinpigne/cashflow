//! Prêts : conditions, tableau d'amortissement et capital restant dû (spec 09, R-PRE-1 à R-PRE-6, ADR-0020).
//!
//! Cas couvert : prêt amortissable à taux fixe et mensualités constantes. Le tableau est calculé à la
//! volée, jamais stocké. Les montants sont en unités mineures (centimes) ; les calculs intermédiaires
//! sont décimaux et arrondis au centime, demi vers le haut (ADR-0001), aux points fixés par R-PRE-3.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::recurrence;
use crate::shared::{Date, DomainError, Money, Rate};

/// Durée maximale d'un prêt, en mois (50 ans).
pub const MAX_DURATION_MONTHS: u32 = 600;
/// Écart de montant toléré entre une mensualité payée et l'échéance (R-PRE-6) : 1 €.
pub const AMOUNT_TOLERANCE: Money = Money::from_minor(100);
/// Écart de date toléré entre une mensualité payée et l'échéance (R-PRE-6).
pub const DATE_TOLERANCE_DAYS: i64 = 5;

/// Assurance emprunteur (R-PRE-2).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Insurance {
    None,
    /// Montant mensuel fixe.
    Fixed {
        amount_minor: i64,
    },
    /// Taux annuel appliqué au capital emprunté.
    RateOnPrincipal {
        rate_ppm: i64,
    },
}

/// Type d'un prêt (R-PRE-1).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoanKind {
    Mortgage,
    Consumer,
    Other,
}

/// Mode de suivi d'un prêt (R-PRE-1) : avec tableau d'amortissement, ou capital restant dû saisi.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LoanMode {
    Schedule,
    Manual,
}

/// Comment se connaît le capital restant dû d'un prêt (R-PRE-4).
#[derive(Debug, Clone, Copy)]
pub enum Tracking<'a> {
    /// Selon le tableau d'amortissement.
    Schedule(&'a [Installment]),
    /// Selon les capitaux restants dus saisis, datés.
    Manual(&'a [(Date, Money)]),
}

/// Période d'un prêt : du déblocage jusqu'à la date où il est soldé (exclue).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoanPeriod {
    pub principal: Money,
    pub start_date: Date,
    pub closed_on: Option<Date>,
}

impl LoanPeriod {
    /// Vérifie le capital et les dates d'un prêt (R-PRE-1).
    pub fn validate(&self, first_installment: Option<Date>) -> Result<(), DomainError> {
        if !self.principal.is_positive()
            || self.closed_on.is_some_and(|d| d < self.start_date)
            || first_installment.is_some_and(|d| d < self.start_date)
        {
            return Err(DomainError::InvalidLoan);
        }
        Ok(())
    }

    /// Capital restant dû à une date (R-PRE-4), positif ; rien avant le déblocage ni une fois soldé.
    pub fn outstanding_at(&self, tracking: Tracking<'_>, date: Date) -> Option<Money> {
        if date < self.start_date || self.closed_on.is_some_and(|closed| date >= closed) {
            return None;
        }
        Some(match tracking {
            Tracking::Schedule(schedule) => {
                schedule.iter().take_while(|i| i.date <= date).last().map_or(self.principal, |i| i.remaining)
            }
            Tracking::Manual(balances) => balances
                .iter()
                .filter(|(d, _)| *d <= date)
                .max_by_key(|(d, _)| *d)
                .map_or(self.principal, |(_, outstanding)| *outstanding),
        })
    }
}

/// Conditions d'un prêt, ce qui détermine son tableau d'amortissement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoanTerms {
    /// Capital emprunté, positif.
    pub principal: Money,
    /// Taux nominal annuel, hors assurance.
    pub annual_rate: Rate,
    pub duration_months: u32,
    pub first_installment: Date,
    pub insurance: Insurance,
}

impl LoanTerms {
    /// Vérifie les conditions (R-PRE-2).
    pub fn validate(&self) -> Result<(), DomainError> {
        let insurance_ok = match self.insurance {
            Insurance::None => true,
            Insurance::Fixed { amount_minor } => amount_minor >= 0,
            Insurance::RateOnPrincipal { rate_ppm } => rate_ppm >= 0,
        };
        if !self.principal.is_positive()
            || self.annual_rate.ppm() < 0
            || self.duration_months == 0
            || self.duration_months > MAX_DURATION_MONTHS
            || !insurance_ok
        {
            return Err(DomainError::InvalidLoanTerms);
        }
        Ok(())
    }

    /// Taux mensuel : taux annuel / 12.
    fn monthly_rate(&self) -> Decimal {
        self.annual_rate.to_fraction() / Decimal::from(12)
    }

    /// Mensualité hors assurance (R-PRE-3) : `P·t / (1 − (1 + t)^−n)`, ou `P / n` si le taux est nul.
    pub fn monthly_payment(&self) -> Money {
        let principal = Decimal::from(self.principal.minor());
        let months = Decimal::from(self.duration_months);
        let t = self.monthly_rate();
        if t.is_zero() {
            return round(principal / months);
        }
        let mut growth = Decimal::ONE;
        for _ in 0..self.duration_months {
            growth *= Decimal::ONE + t;
        }
        round(principal * t * growth / (growth - Decimal::ONE))
    }

    /// Assurance de chaque échéance (R-PRE-3).
    pub fn monthly_insurance(&self) -> Money {
        match self.insurance {
            Insurance::None => Money::ZERO,
            Insurance::Fixed { amount_minor } => Money::from_minor(amount_minor),
            Insurance::RateOnPrincipal { rate_ppm } => round(
                Decimal::from(self.principal.minor()) * Rate::from_ppm(rate_ppm).to_fraction() / Decimal::from(12),
            ),
        }
    }
}

/// Une ligne du tableau d'amortissement. Montants positifs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installment {
    /// Numéro, à partir de 1.
    pub number: u32,
    pub date: Date,
    /// Mensualité hors assurance : capital + intérêts.
    pub payment: Money,
    pub principal: Money,
    pub interest: Money,
    pub insurance: Money,
    /// Capital restant dû après cette échéance.
    pub remaining: Money,
}

impl Installment {
    /// Montant prélevé : mensualité + assurance.
    pub fn total(&self) -> Money {
        self.payment + self.insurance
    }
}

fn round(value: Decimal) -> Money {
    let rounded = value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    Money::from_minor(rounded.to_i64().unwrap_or_default())
}

/// Tableau d'amortissement (R-PRE-3). La dernière échéance solde le capital restant dû : sa mensualité
/// absorbe l'écart d'arrondi.
pub fn schedule(terms: &LoanTerms) -> Result<Vec<Installment>, DomainError> {
    terms.validate()?;
    let payment = terms.monthly_payment();
    let insurance = terms.monthly_insurance();
    let t = terms.monthly_rate();
    let mut remaining = terms.principal;
    let mut result = Vec::with_capacity(terms.duration_months as usize);
    for number in 1..=terms.duration_months {
        let date =
            recurrence::add_months_clamped(terms.first_installment, number - 1).ok_or(DomainError::InvalidLoanTerms)?;
        let interest = round(Decimal::from(remaining.minor()) * t);
        let principal = if number == terms.duration_months { remaining } else { payment - interest };
        // Une mensualité inférieure aux intérêts (taux extrême) ne rembourserait rien : conditions refusées.
        if principal.is_negative() {
            return Err(DomainError::InvalidLoanTerms);
        }
        remaining -= principal;
        result.push(Installment {
            number,
            date,
            payment: principal + interest,
            principal,
            interest,
            insurance,
            remaining,
        });
    }
    Ok(result)
}

/// Capital restant dû théorique à une date (R-PRE-3) : après la dernière échéance datée au plus tard
/// ce jour-là ; le capital emprunté avant la première.
pub fn outstanding_at(terms: &LoanTerms, schedule: &[Installment], date: Date) -> Money {
    schedule.iter().take_while(|i| i.date <= date).last().map_or(terms.principal, |i| i.remaining)
}

/// Échéance à laquelle correspond un débit du compte payeur (R-PRE-6) : montant égal à l'échéance totale
/// à ±1 €, date à ±5 jours, pas encore payée. La plus proche en date l'emporte.
pub fn matching_installment<'a>(
    schedule: &'a [Installment],
    paid: &[u32],
    debit: Money,
    date: Date,
) -> Option<&'a Installment> {
    let amount = -debit;
    schedule
        .iter()
        .filter(|i| !paid.contains(&i.number))
        .filter(|i| (i.total() - amount).abs() <= AMOUNT_TOLERANCE)
        .filter(|i| (i.date - date).num_days().abs() <= DATE_TOLERANCE_DAYS)
        .min_by_key(|i| (i.date - date).num_days().abs())
}

/// Répartition d'une mensualité payée (R-PRE-6). Montants positifs, de somme égale au montant payé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaymentParts {
    pub principal: Money,
    pub interest: Money,
    pub insurance: Money,
}

/// Répartit un débit selon une échéance : le capital est celui du tableau ; l'écart de montant (au plus
/// 1 €) est porté par l'assurance, ou par les intérêts sans assurance (R-PRE-6).
pub fn payment_parts(installment: &Installment, debit: Money) -> Result<PaymentParts, DomainError> {
    let amount = -debit;
    let gap = amount - installment.total();
    if gap.abs() > AMOUNT_TOLERANCE {
        return Err(DomainError::InstallmentMismatch);
    }
    let mut parts = PaymentParts {
        principal: installment.principal,
        interest: installment.interest,
        insurance: installment.insurance,
    };
    if installment.insurance.is_positive() {
        parts.insurance += gap;
    } else {
        parts.interest += gap;
    }
    if parts.interest.is_negative() || parts.insurance.is_negative() {
        return Err(DomainError::InstallmentMismatch);
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn m(minor: i64) -> Money {
        Money::from_minor(minor)
    }

    /// Prêt de la spec (PRE-02) : 200 000 € à 3 % sur 240 mois, assurance 0,30 % du capital.
    fn reference() -> LoanTerms {
        LoanTerms {
            principal: m(20_000_000),
            annual_rate: Rate::from_ppm(30_000),
            duration_months: 240,
            first_installment: d("2026-01-05"),
            insurance: Insurance::RateOnPrincipal { rate_ppm: 3_000 },
        }
    }

    #[test]
    fn pre_02_r_pre_3_reference_schedule() {
        let terms = reference();
        assert_eq!(terms.monthly_payment(), m(110_920));
        let rows = schedule(&terms).unwrap();
        assert_eq!(rows.len(), 240);
        let first = rows[0];
        assert_eq!((first.interest, first.principal, first.insurance), (m(50_000), m(60_920), m(5_000)));
        assert_eq!(first.total(), m(115_920));
        assert_eq!(first.remaining, m(19_939_080));
        assert_eq!(rows[1].date, d("2026-02-05"));
        assert_eq!(rows[239].date, d("2045-12-05"));
    }

    #[test]
    fn r_pre_3_principal_sums_to_amount_borrowed_and_last_installment_settles() {
        let terms = reference();
        let rows = schedule(&terms).unwrap();
        assert_eq!(rows.iter().map(|i| i.principal).sum::<Money>(), terms.principal);
        assert_eq!(rows.last().unwrap().remaining, Money::ZERO);
        // Seule la dernière mensualité s'écarte : l'arrondi de la mensualité (110 919,52 → 110 920 centimes),
        // capitalisé sur 240 mois, la réduit de 1,57 €.
        assert!(rows[..239].iter().all(|i| i.payment == m(110_920)));
        assert_eq!(rows[239].payment, m(110_763));
    }

    #[test]
    fn r_pre_3_zero_rate_and_fixed_insurance() {
        let terms = LoanTerms {
            principal: m(1_000_000),
            annual_rate: Rate::ZERO,
            duration_months: 3,
            first_installment: d("2026-01-31"),
            insurance: Insurance::Fixed { amount_minor: 1_250 },
        };
        let rows = schedule(&terms).unwrap();
        assert_eq!(rows.iter().map(|i| i.payment.minor()).collect::<Vec<_>>(), [333_333, 333_333, 333_334]);
        assert!(rows.iter().all(|i| i.interest.is_zero() && i.insurance == m(1_250)));
        // Le 31 n'existe pas en février : l'échéance tombe le dernier jour du mois (comme R-TRX-41).
        assert_eq!(rows[1].date, d("2026-02-28"));
    }

    #[test]
    fn r_pre_2_invalid_terms_are_refused() {
        let mut terms = reference();
        terms.duration_months = 0;
        assert_eq!(schedule(&terms), Err(DomainError::InvalidLoanTerms));
        terms = reference();
        terms.principal = Money::ZERO;
        assert_eq!(terms.validate(), Err(DomainError::InvalidLoanTerms));
        terms = reference();
        terms.insurance = Insurance::Fixed { amount_minor: -1 };
        assert_eq!(terms.validate(), Err(DomainError::InvalidLoanTerms));
    }

    #[test]
    fn r_pre_4_outstanding_principal_at_a_date() {
        let terms = reference();
        let rows = schedule(&terms).unwrap();
        assert_eq!(outstanding_at(&terms, &rows, d("2026-01-04")), m(20_000_000));
        assert_eq!(outstanding_at(&terms, &rows, d("2026-01-05")), m(19_939_080));
        assert_eq!(outstanding_at(&terms, &rows, d("2026-02-04")), m(19_939_080));
        assert_eq!(outstanding_at(&terms, &rows, d("2050-01-01")), Money::ZERO);
    }

    #[test]
    fn r_pre_6_matching_tolerates_one_euro_and_five_days() {
        let rows = schedule(&reference()).unwrap();
        let found = matching_installment(&rows, &[], m(-115_920), d("2026-02-07")).unwrap();
        assert_eq!(found.number, 2);
        assert_eq!(matching_installment(&rows, &[], m(-116_020), d("2026-02-05")).unwrap().number, 2);
        assert!(matching_installment(&rows, &[], m(-116_021), d("2026-02-05")).is_none());
        assert!(matching_installment(&rows, &[], m(-115_920), d("2026-02-11")).is_none());
        // Une échéance déjà payée n'est plus proposée.
        assert!(matching_installment(&rows, &[2], m(-115_920), d("2026-02-05")).is_none());
    }

    #[test]
    fn pre_03_r_pre_6_payment_parts_and_gap_on_insurance() {
        let rows = schedule(&reference()).unwrap();
        let parts = payment_parts(&rows[0], m(-115_920)).unwrap();
        assert_eq!(parts, PaymentParts { principal: m(60_920), interest: m(50_000), insurance: m(5_000) });
        // 40 centimes de plus : portés par l'assurance, le capital reste celui du tableau.
        let parts = payment_parts(&rows[0], m(-115_960)).unwrap();
        assert_eq!(parts, PaymentParts { principal: m(60_920), interest: m(50_000), insurance: m(5_040) });
        assert_eq!(payment_parts(&rows[0], m(-117_000)), Err(DomainError::InstallmentMismatch));
    }

    #[test]
    fn pre_02_r_pre_4_outstanding_in_both_modes() {
        let rows = schedule(&reference()).unwrap();
        let period = LoanPeriod { principal: m(20_000_000), start_date: d("2025-12-15"), closed_on: None };
        assert_eq!(period.outstanding_at(Tracking::Schedule(&rows), d("2025-12-14")), None);
        assert_eq!(period.outstanding_at(Tracking::Schedule(&rows), d("2026-01-04")), Some(m(20_000_000)));
        assert_eq!(period.outstanding_at(Tracking::Schedule(&rows), d("2026-01-05")), Some(m(19_939_080)));
        let balances = [(d("2026-03-31"), m(15_000_000))];
        assert_eq!(period.outstanding_at(Tracking::Manual(&balances), d("2026-03-30")), Some(m(20_000_000)));
        assert_eq!(period.outstanding_at(Tracking::Manual(&balances), d("2026-04-01")), Some(m(15_000_000)));
        let closed = LoanPeriod { closed_on: Some(d("2026-06-01")), ..period };
        assert_eq!(closed.outstanding_at(Tracking::Manual(&balances), d("2026-06-01")), None);
        assert_eq!(period.validate(Some(d("2025-12-01"))), Err(DomainError::InvalidLoan));
    }

    #[test]
    fn r_pre_6_gap_on_interest_without_insurance() {
        let mut terms = reference();
        terms.insurance = Insurance::None;
        let rows = schedule(&terms).unwrap();
        let parts = payment_parts(&rows[0], m(-110_900)).unwrap();
        assert_eq!(parts, PaymentParts { principal: m(60_920), interest: m(49_980), insurance: Money::ZERO });
    }
}
