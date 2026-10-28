//! Relevés de compte archivés (CPT-11, ADR-0024).

use chrono::{Datelike, Months};

use crate::documents::{self, MediaType};
use crate::salaries::month_of;
use crate::shared::{Date, DomainError};

/// Vérifie le mois et la période d'un relevé : un premier du mois, une période dans l'ordre (R-CPT-18).
pub fn check(statement_month: Date, period: Option<(Date, Date)>) -> Result<(), DomainError> {
    if statement_month.day() != 1 || period.is_some_and(|(start, end)| start > end) {
        return Err(DomainError::InvalidStatement);
    }
    Ok(())
}

/// Vérifie le fichier d'un relevé : un document valide (ADR-0024), et un PDF (R-CPT-18).
pub fn check_file(content: &[u8]) -> Result<(), DomainError> {
    match documents::check(content)? {
        MediaType::Pdf => Ok(()),
        MediaType::Jpeg | MediaType::Png => Err(DomainError::StatementNotPdf),
    }
}

/// Mois d'un relevé importé (R-IMP-28) : celui qui couvre le plus de jours de la période (le plus ancien à
/// égalité) ; sans période, celui de la date de fin du lot, à défaut celui de la validation.
pub fn month_of_import(period: Option<(Date, Date)>, end: Option<Date>, committed_on: Date) -> Date {
    let Some((start, last)) = period.filter(|(start, last)| start <= last) else {
        return month_of(end.unwrap_or(committed_on));
    };
    let (mut best, mut best_days) = (month_of(start), 0);
    let mut month = month_of(start);
    while month <= last {
        let Some(next) = month.checked_add_months(Months::new(1)) else { break };
        let from = start.max(month);
        let to = last.min(next.pred_opt().unwrap_or(next));
        let days = (to - from).num_days() + 1;
        if days > best_days {
            (best, best_days) = (month, days);
        }
        month = next;
    }
    best
}

/// Mois sans relevé, du plus récent au plus ancien : mois sans aucun relevé, à partir du mois du premier relevé
/// et jusqu'au mois de clôture du compte ou au mois précédant le mois courant, le plus tôt des deux (R-CPT-21).
pub fn missing_months(statement_months: &[Date], closed_on: Option<Date>, today: Date) -> Vec<Date> {
    let Some(first) = statement_months.iter().min().copied() else {
        return Vec::new();
    };
    let last = month_of(today).checked_sub_months(Months::new(1)).unwrap_or(today);
    let last = closed_on.map_or(last, |closed| last.min(month_of(closed)));
    let mut missing = Vec::new();
    let mut month = month_of(first);
    while month <= last {
        if !statement_months.contains(&month) {
            missing.push(month);
        }
        let Some(next) = month.checked_add_months(Months::new(1)) else { break };
        month = next;
    }
    missing.reverse();
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn r_cpt_18_statement_month_is_a_first_and_period_is_ordered() {
        assert_eq!(check(d("2026-09-01"), Some((d("2026-09-01"), d("2026-09-30")))), Ok(()));
        assert_eq!(check(d("2026-09-01"), None), Ok(()));
        assert_eq!(check(d("2026-09-15"), None), Err(DomainError::InvalidStatement));
        assert_eq!(
            check(d("2026-09-01"), Some((d("2026-09-30"), d("2026-09-01")))),
            Err(DomainError::InvalidStatement)
        );
    }

    #[test]
    fn r_cpt_18_statement_file_is_a_pdf() {
        assert_eq!(check_file(b"%PDF-1.7 releve"), Ok(()));
        assert_eq!(check_file(&[0xFF, 0xD8, 0xFF, 0xE0]), Err(DomainError::StatementNotPdf));
        assert_eq!(check_file(b"PK\x03\x04"), Err(DomainError::UnsupportedDocumentType));
        assert_eq!(check_file(b""), Err(DomainError::EmptyDocument));
    }

    #[test]
    fn r_imp_28_imported_statement_month_covers_most_of_the_period() {
        let today = d("2026-10-20");
        let lcl = (d("2026-09-03"), d("2026-10-02"));
        assert_eq!(month_of_import(Some(lcl), Some(d("2026-10-02")), today), d("2026-09-01"));
        let december = (d("2025-12-03"), d("2026-01-02"));
        assert_eq!(month_of_import(Some(december), None, today), d("2025-12-01"));
        let calendar = (d("2026-09-01"), d("2026-09-30"));
        assert_eq!(month_of_import(Some(calendar), None, today), d("2026-09-01"));
        // À égalité (16 jours chacun), le plus ancien.
        assert_eq!(month_of_import(Some((d("2026-08-16"), d("2026-09-16"))), None, today), d("2026-08-01"));
        assert_eq!(month_of_import(None, Some(d("2026-09-30")), today), d("2026-09-01"));
        assert_eq!(month_of_import(None, None, today), d("2026-10-01"));
    }

    #[test]
    fn r_cpt_21_missing_months_start_at_the_first_statement() {
        let months = [d("2026-06-01"), d("2026-07-01"), d("2026-09-01"), d("2026-09-01")];
        assert_eq!(missing_months(&months, None, d("2026-10-20")), vec![d("2026-08-01")]);
        assert!(missing_months(&[], None, d("2026-10-20")).is_empty());
        // Un relevé en avance ne crée pas de mois manquant au-delà du mois précédent.
        assert!(missing_months(&[d("2026-10-01")], None, d("2026-10-20")).is_empty());
    }

    #[test]
    fn r_cpt_21_missing_months_stop_at_the_closing_month() {
        let missing = missing_months(&[d("2026-02-01")], Some(d("2026-05-12")), d("2026-10-20"));
        assert_eq!(missing, vec![d("2026-05-01"), d("2026-04-01"), d("2026-03-01")]);
    }
}
