//! Lecture des dates (R-IMP-4).

use chrono::{Datelike, NaiveDate};

use crate::shared::Date;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DateError {
    #[error("date vide")]
    Empty,
    #[error("date illisible : « {0} »")]
    Unreadable(String),
    #[error("format de date inconnu : « {0} »")]
    UnknownFormat(String),
}

/// Format de date d'un profil : `dd/MM/yyyy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd.MM`, `MM/dd/yyyy`…
/// Les séparateurs sont libres ; `yy` désigne une année à deux chiffres, une absence d'année demande
/// la période du relevé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateFormat {
    order: Vec<Part>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Day,
    Month,
    Year4,
    Year2,
}

impl DateFormat {
    pub fn parse(pattern: &str) -> Result<Self, DateError> {
        let mut order = Vec::new();
        let tokens: Vec<&str> = pattern.split(|c: char| !c.is_ascii_alphabetic()).filter(|t| !t.is_empty()).collect();
        for token in tokens {
            order.push(match token {
                "dd" | "d" => Part::Day,
                "MM" | "M" => Part::Month,
                "yyyy" | "YYYY" => Part::Year4,
                "yy" | "YY" => Part::Year2,
                _ => return Err(DateError::UnknownFormat(pattern.to_owned())),
            });
        }
        let has = |p: Part| order.contains(&p);
        if !has(Part::Day) || !has(Part::Month) || order.len() > 3 {
            return Err(DateError::UnknownFormat(pattern.to_owned()));
        }
        Ok(Self { order })
    }

    pub fn has_year(&self) -> bool {
        self.order.iter().any(|p| matches!(p, Part::Year2 | Part::Year4))
    }
}

/// Contexte d'interprétation des années incomplètes (R-IMP-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearContext {
    pub today: Date,
    /// Période du relevé, si elle est connue : donne l'année des dates sans année.
    pub period: Option<(Date, Date)>,
}

/// Lit une date selon `format`. Une année à deux chiffres prend le siècle le plus proche d'aujourd'hui ;
/// une date sans année prend l'année de la période du relevé, le mois départageant un relevé à cheval
/// sur deux années (R-IMP-4).
pub fn parse_date(raw: &str, format: &DateFormat, context: YearContext) -> Result<Date, DateError> {
    let text = raw.trim();
    if text.is_empty() {
        return Err(DateError::Empty);
    }
    let numbers: Vec<&str> = text.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).collect();
    if numbers.len() != format.order.len() {
        return Err(DateError::Unreadable(raw.to_owned()));
    }
    let (mut day, mut month, mut year) = (None, None, None);
    for (part, value) in format.order.iter().zip(&numbers) {
        let n: i32 = value.parse().map_err(|_| DateError::Unreadable(raw.to_owned()))?;
        match part {
            Part::Day => day = u32::try_from(n).ok(),
            Part::Month => month = u32::try_from(n).ok(),
            Part::Year4 => year = Some(n),
            Part::Year2 => year = Some(nearest_century(n, context.today)),
        }
    }
    let (day, month) = day.zip(month).ok_or_else(|| DateError::Unreadable(raw.to_owned()))?;
    let year = match year {
        Some(year) => year,
        None => year_from_period(month, context).ok_or_else(|| DateError::Unreadable(raw.to_owned()))?,
    };
    NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| DateError::Unreadable(raw.to_owned()))
}

fn nearest_century(two_digits: i32, today: Date) -> i32 {
    let century = today.year() - today.year().rem_euclid(100);
    [century - 100, century, century + 100]
        .into_iter()
        .map(|c| c + two_digits)
        .min_by_key(|y| (y - today.year()).abs())
        .unwrap_or(century + two_digits)
}

fn year_from_period(month: u32, context: YearContext) -> Option<i32> {
    let (start, end) = context.period?;
    if start.year() == end.year() {
        return Some(start.year());
    }
    // Relevé à cheval (décembre → janvier) : les mois de fin de période sont dans l'année de fin.
    Some(if month >= start.month() { start.year() } else { end.year() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn context() -> YearContext {
        YearContext { today: d("2026-10-20"), period: None }
    }

    #[test]
    fn reads_common_formats() {
        let fr = DateFormat::parse("dd/MM/yyyy").unwrap();
        assert_eq!(parse_date("12/03/2026", &fr, context()), Ok(d("2026-03-12")));
        let iso = DateFormat::parse("yyyy-MM-dd").unwrap();
        assert_eq!(parse_date("2026-03-12", &iso, context()), Ok(d("2026-03-12")));
        assert!(parse_date("31/02/2026", &fr, context()).is_err());
        assert!(DateFormat::parse("MM/yyyy").is_err());
    }

    #[test]
    fn r_imp_4_two_digit_year_takes_nearest_century() {
        let format = DateFormat::parse("dd/MM/yy").unwrap();
        assert_eq!(parse_date("12/03/26", &format, context()), Ok(d("2026-03-12")));
        assert_eq!(parse_date("12/03/99", &format, context()), Ok(d("1999-03-12")));
    }

    #[test]
    fn r_imp_4_year_from_statement_period() {
        let format = DateFormat::parse("dd.MM").unwrap();
        assert!(!format.has_year());
        let straddling = YearContext { today: d("2026-10-20"), period: Some((d("2025-12-15"), d("2026-01-14"))) };
        assert_eq!(parse_date("28.12", &format, straddling), Ok(d("2025-12-28")));
        assert_eq!(parse_date("03.01", &format, straddling), Ok(d("2026-01-03")));
        assert!(parse_date("03.01", &format, context()).is_err());
    }
}
