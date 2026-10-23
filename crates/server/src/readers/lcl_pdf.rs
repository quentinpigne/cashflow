//! Relevé de compte LCL en PDF (IMP-10, R-IMP-23 à R-IMP-26).
//!
//! Le texte extrait localement avec sa position (`readers::pdf`) est regroupé en lignes ; la table des
//! opérations est repérée par ses en-têtes, dont l'abscisse donne les colonnes.

use cashflow_domain::import::amount::{AmountFormat, parse_amount};
use cashflow_domain::import::date::{DateFormat, YearContext, parse_date};
use cashflow_domain::{Date, Money};

use super::pdf::{self, TextItem};
use super::{ReadContext, ReadError, ReadLine, Statement, StatementReader};

/// Écart vertical maximal entre deux textes d'une même ligne, en points.
const SAME_ROW: f64 = 1.5;

/// Le texte suit `pattern`, où `9` désigne un chiffre et tout autre caractère lui-même.
fn shaped(text: &str, pattern: &str) -> bool {
    text.chars().count() == pattern.chars().count()
        && text.chars().zip(pattern.chars()).all(|(t, p)| if p == '9' { t.is_ascii_digit() } else { t == p })
}

/// Montant d'un relevé : « 1 234,56 », milliers séparés par une espace, deux décimales.
fn is_amount(text: &str) -> bool {
    let Some((units, cents)) = text.strip_prefix('-').unwrap_or(text).split_once(',') else { return false };
    let groups: Vec<&str> = units.split(' ').collect();
    shaped(cents, "99")
        && groups.iter().enumerate().all(|(i, g)| {
            !g.is_empty() && g.chars().all(|c| c.is_ascii_digit()) && (if i == 0 { g.len() <= 3 } else { g.len() == 3 })
        })
}

/// Période du relevé : « du 03.09.2026 au 02.10.2026 » (R-IMP-25).
fn period(text: &str) -> Option<(&str, &str)> {
    let rest = &text[text.find("du ")? + 3..];
    let (start, rest) = (rest.get(..10)?, rest.get(10..)?);
    let end = rest.strip_prefix(" au ")?.get(..10)?;
    (shaped(start, "99.99.9999") && shaped(end, "99.99.9999")).then_some((start, end))
}

/// Date d'achat d'un paiement par carte : « CB LIDL 05/09/26 » (R-IMP-26).
fn card_date(label: &str) -> Option<&str> {
    let date = label.strip_prefix("CB ")?.rsplit(' ').next()?;
    shaped(date, "99/99/99").then_some(date)
}

fn is_footer(label: &str) -> bool {
    label.strip_prefix("Page ").is_some_and(|rest| rest.contains(" / "))
}

/// Lecteur des relevés de compte LCL.
pub struct LclPdfReader;

impl StatementReader for LclPdfReader {
    fn read(&self, content: &[u8], context: ReadContext) -> Result<Statement, ReadError> {
        parse(&pdf::text_items(content)?, context)
    }
}

/// Une ligne visuelle : les textes d'une même page à la même hauteur, de gauche à droite.
struct Row<'a> {
    items: Vec<&'a TextItem>,
}

fn rows(items: &[TextItem]) -> Vec<Row<'_>> {
    let mut sorted: Vec<&TextItem> = items.iter().collect();
    sorted.sort_by(|a, b| a.page.cmp(&b.page).then(b.y.total_cmp(&a.y)).then(a.first_x.total_cmp(&b.first_x)));
    let mut rows: Vec<Row<'_>> = Vec::new();
    for item in sorted {
        match rows.last_mut() {
            Some(row) if row.items[0].page == item.page && (row.items[0].y - item.y).abs() <= SAME_ROW => {
                row.items.push(item);
            }
            _ => rows.push(Row { items: vec![item] }),
        }
    }
    for row in &mut rows {
        row.items.sort_by(|a, b| a.first_x.total_cmp(&b.first_x));
    }
    rows
}

/// Abscisses des en-têtes de la table des opérations (R-IMP-24).
#[derive(Debug, Clone, Copy)]
struct Columns {
    date: f64,
    value: f64,
    /// Frontière entre les colonnes `DEBIT` et `CREDIT`, comparée au dernier caractère d'un montant.
    debit_credit: f64,
}

fn header(row: &Row<'_>) -> Option<Columns> {
    let at = |name: &str| row.items.iter().find(|i| i.text == name);
    let (date, _, value, debit, credit) = (at("DATE")?, at("LIBELLE")?, at("VALEUR")?, at("DEBIT")?, at("CREDIT")?);
    let center = |i: &TextItem| f64::midpoint(i.first_x, i.last_x);
    Some(Columns {
        date: date.first_x,
        value: value.first_x,
        debit_credit: f64::midpoint(center(debit), center(credit)),
    })
}

/// Contenu d'une ligne de la table, rangé par colonne.
#[derive(Default)]
struct Cells {
    date: Option<String>,
    label: Vec<String>,
    value_date: Option<String>,
    debit: Option<String>,
    credit: Option<String>,
}

impl Cells {
    fn of(row: &Row<'_>, columns: Columns) -> Self {
        let mut cells = Cells::default();
        for item in &row.items {
            let text = item.text.clone();
            if shaped(&text, "99.99") && item.first_x < columns.date + 20.0 {
                cells.date = Some(text);
            } else if shaped(&text, "99.99.99") && (item.first_x - columns.value).abs() < 20.0 {
                cells.value_date = Some(text);
            } else if is_amount(&text) && item.first_x > columns.value + 20.0 {
                if item.last_x < columns.debit_credit {
                    cells.debit = Some(text);
                } else {
                    cells.credit = Some(text);
                }
            } else {
                cells.label.push(text);
            }
        }
        cells
    }

    fn label(&self) -> String {
        self.label.join(" ")
    }

    /// Montant signé : crédit positif, débit négatif (R-IMP-24).
    fn amount(&self, context: ReadContext) -> Result<Money, String> {
        let read = |raw: &str| parse_amount(raw, AmountFormat::FRENCH, context.currency).map_err(|e| e.to_string());
        match (&self.debit, &self.credit) {
            (Some(debit), None) => read(debit).map(|m| -m),
            (None, Some(credit)) => read(credit),
            (None, None) => Err("montant absent".into()),
            (Some(_), Some(_)) => Err("montant au débit et au crédit".into()),
        }
    }

    fn raw(&self) -> Vec<String> {
        [&self.date, &Some(self.label.join(" ")), &self.value_date, &self.debit, &self.credit]
            .into_iter()
            .map(|c| c.clone().unwrap_or_default())
            .collect()
    }
}

fn date_format(pattern: &str) -> Result<DateFormat, ReadError> {
    DateFormat::parse(pattern).map_err(|e| ReadError::Unreadable(e.to_string()))
}

/// Relevé lu dans les textes positionnés d'un PDF LCL (R-IMP-23 à R-IMP-26).
pub fn parse(items: &[TextItem], context: ReadContext) -> Result<Statement, ReadError> {
    let not_lcl = || ReadError::Unreadable("ce PDF n'est pas un relevé de compte LCL reconnu".into());
    let rows = rows(items);
    let full = date_format("dd.MM.yyyy")?;
    let day_month = date_format("dd.MM")?;
    let short = date_format("dd.MM.yy")?;
    let card = date_format("dd/MM/yy")?;

    let mut year = YearContext { today: context.today, period: None };
    let (start, end) = items.iter().find_map(|i| period(&i.text)).ok_or_else(not_lcl)?;
    let start = parse_date(start, &full, year).map_err(|e| ReadError::Unreadable(e.to_string()))?;
    let end = parse_date(end, &full, year).map_err(|e| ReadError::Unreadable(e.to_string()))?;
    year.period = Some((start, end));
    let mut statement = Statement { period: Some((start, end)), ..Statement::default() };

    let mut columns: Option<Columns> = None;
    let mut page = 0;
    let mut found_table = false;
    for row in &rows {
        // La table recommence sous les en-têtes de chaque page ; ailleurs, la page est ignorée.
        if row.items[0].page != page {
            page = row.items[0].page;
            columns = None;
        }
        if let Some(found) = header(row) {
            columns = Some(found);
            found_table = true;
            continue;
        }
        let Some(columns) = columns else { continue };
        let cells = Cells::of(row, columns);
        let label = cells.label();
        if is_footer(&label) {
            continue;
        }
        // Soldes et totaux (R-IMP-25).
        if label.starts_with("ANCIEN SOLDE") {
            statement.opening_balance = cells.amount(context).ok();
            continue;
        }
        if label.starts_with("SOLDE EN EUROS") {
            statement.closing_balance = cells.amount(context).ok();
            statement.closing_date = cells.date.as_deref().and_then(|d| parse_date(d, &day_month, year).ok());
            break;
        }
        if label.starts_with("SOLDE INTERMEDIAIRE") || label.starts_with("TOTAUX") {
            continue;
        }
        let has_amount = cells.debit.is_some() || cells.credit.is_some();
        // Sans date ni montant : suite du libellé de l'opération précédente (R-IMP-24).
        if cells.date.is_none() && !has_amount {
            if let Some(previous) = statement.lines.last_mut()
                && !label.is_empty()
            {
                previous.bank_label = format!("{}\n{label}", previous.bank_label);
                if let Some(raw_label) = previous.raw.get_mut(1) {
                    *raw_label = previous.bank_label.clone();
                }
            }
            continue;
        }
        statement.lines.push(operation(
            &cells,
            &label,
            statement.lines.len(),
            context,
            year,
            (&day_month, &short, &card),
        ));
    }
    if !found_table {
        return Err(not_lcl());
    }
    Ok(statement)
}

/// Une opération de la table (R-IMP-24, R-IMP-26).
fn operation(
    cells: &Cells,
    label: &str,
    index: usize,
    context: ReadContext,
    year: YearContext,
    (day_month, short, card): (&DateFormat, &DateFormat, &DateFormat),
) -> ReadLine {
    let mut errors = Vec::new();
    let date: Option<Date> = match &cells.date {
        Some(d) => parse_date(d, day_month, year).map_err(|e| errors.push(e.to_string())).ok(),
        None => {
            errors.push("date absente".into());
            None
        }
    };
    let value_date = cells.value_date.as_deref().and_then(|d| parse_date(d, short, year).ok());
    let purchase_date = card_date(label).and_then(|c| parse_date(c, card, year).ok());
    let amount = cells.amount(context).map_err(|e| errors.push(e)).ok();
    if label.is_empty() {
        errors.push("libellé vide".into());
    }
    ReadLine {
        line_number: i64::try_from(index + 1).unwrap_or(i64::MAX),
        raw: cells.raw(),
        date,
        value_date,
        amount,
        bank_label: label.to_owned(),
        fitid: None,
        bank_category: None,
        purchase_date,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use cashflow_domain::Currency;

    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    /// Un texte comme le place un relevé LCL : `x` est l'abscisse du premier caractère, et une police à
    /// chasse fixe de 6 points par caractère donne celle du dernier.
    fn item(page: u32, y: f64, x: f64, text: &str) -> TextItem {
        let width = 6.0 * (text.chars().count().saturating_sub(1) as f64);
        TextItem { page, y, first_x: x, last_x: x + width, text: text.into() }
    }

    /// Un montant aligné à droite sur `right` (colonne DEBIT à 460, CREDIT à 532).
    fn amount(page: u32, y: f64, right: f64, text: &str) -> TextItem {
        let width = 6.0 * (text.chars().count().saturating_sub(1) as f64);
        TextItem { page, y, first_x: right - width, last_x: right, text: text.into() }
    }

    fn header(page: u32, y: f64) -> Vec<TextItem> {
        vec![
            item(page, y, 41.6, "DATE"),
            item(page, y, 203.0, "LIBELLE"),
            item(page, y, 370.0, "VALEUR"),
            item(page, y, 438.0, "DEBIT"),
            item(page, y, 510.0, "CREDIT"),
        ]
    }

    fn op(
        page: u32,
        y: f64,
        date: &str,
        label: &str,
        value: &str,
        debit: Option<&str>,
        credit: Option<&str>,
    ) -> Vec<TextItem> {
        let mut row = vec![item(page, y, 42.5, date), item(page, y, 74.8, label), item(page, y, 365.6, value)];
        if let Some(debit) = debit {
            row.push(amount(page, y, 460.0, debit));
        }
        if let Some(credit) = credit {
            row.push(amount(page, y, 532.0, credit));
        }
        row
    }

    fn context() -> ReadContext {
        ReadContext { today: d("2026-10-23"), currency: Currency::EUR }
    }

    /// Un relevé fictif sur deux pages, à cheval sur deux années.
    fn statement() -> Vec<TextItem> {
        let mut items = vec![
            item(1, 797.8, 131.8, "RELEVE DE COMPTE"),
            item(1, 766.6, 346.3, "du 03.12.2025 au 02.01.2026 -  N° 12"),
            item(1, 587.4, 47.1, "30002"),
        ];
        items.extend(header(2, 671.0));
        items.push(item(2, 647.1, 41.9, "02.12"));
        items.push(item(2, 647.1, 268.6, "ANCIEN SOLDE"));
        items.push(amount(2, 647.1, 532.0, "1 250,00"));
        items.extend(op(2, 623.2, "04.12", "VIREMENT EMPLOYEUR", "04.12.25", None, Some("2 400,00")));
        items.push(item(2, 611.2, 80.8, "LIBELLE:SALAIRE NOVEMBRE"));
        items.push(item(2, 599.3, 80.8, "REF.CLIENT:123"));
        items.extend(op(2, 587.4, "05.12", "CB BOULANGERIE 03/12/25", "05.12.25", Some("4,20"), None));
        items.extend(op(2, 575.4, "06.12", "CB Carte Ticket Res 04/12/25", "06.12.25", Some("12,50"), None));
        items.push(item(2, 563.5, 80.8, "Montrouge      EUR         3,72"));
        items.push(item(2, 37.0, 519.6, "Page 2 / 3"));
        items.extend(header(3, 671.0));
        items.extend(op(3, 647.1, "31.12", "PRLV SEPA ENERGIE", "31.12.25", Some("1 016,00"), None));
        items.push(item(3, 623.2, 92.8, "SOLDE INTERMEDIAIRE A FIN DECEMBRE"));
        items.push(amount(3, 623.2, 532.0, "2 617,30"));
        items.extend(op(3, 611.2, "02.01", "CB LIBRAIRIE 31/12/25", "02.01.26", Some("20,00"), None));
        items.push(item(3, 587.4, 316.5, "TOTAUX"));
        items.push(amount(3, 587.4, 460.0, "1 052,70"));
        items.push(amount(3, 587.4, 532.0, "3 650,00"));
        items.push(item(3, 563.5, 41.9, "02.01"));
        items.push(item(3, 563.5, 263.4, "SOLDE EN EUROS"));
        items.push(amount(3, 563.5, 532.0, "2 597,30"));
        // Après le solde de fin : informations réglementaires, ignorées (R-IMP-25).
        items.push(item(3, 480.0, 47.9, "Votre autorisation de découvert au 15/12/2025."));
        items.push(amount(3, 470.0, 532.0, "500,00"));
        items
    }

    #[test]
    fn r_imp_24_r_imp_25_reads_operations_and_balances() {
        let s = parse(&statement(), context()).unwrap();
        assert_eq!(s.period, Some((d("2025-12-03"), d("2026-01-02"))));
        assert_eq!(s.opening_balance, Some(Money::from_minor(125_000)));
        assert_eq!(s.closing_balance, Some(Money::from_minor(259_730)));
        assert_eq!(s.closing_date, Some(d("2026-01-02")));
        let amounts: Vec<i64> = s.lines.iter().map(|l| l.amount.unwrap().minor()).collect();
        assert_eq!(amounts, [240_000, -420, -1_250, -101_600, -2_000]);
        assert!(s.lines.iter().all(|l| l.errors.is_empty()));
        // Le contrôle du relevé tombe juste (R-IMP-18).
        let total: Money = s.lines.iter().filter_map(|l| l.amount).sum();
        assert_eq!(s.opening_balance.unwrap() + total, s.closing_balance.unwrap());
    }

    #[test]
    fn r_imp_24_continuation_lines_extend_the_label() {
        let s = parse(&statement(), context()).unwrap();
        assert_eq!(s.lines[0].bank_label, "VIREMENT EMPLOYEUR\nLIBELLE:SALAIRE NOVEMBRE\nREF.CLIENT:123");
        assert_eq!(s.lines[2].bank_label, "CB Carte Ticket Res 04/12/25\nMontrouge      EUR         3,72");
        assert_eq!(s.lines[2].amount, Some(Money::from_minor(-1_250)), "le montant du libellé n'est pas lu");
        assert_eq!(s.lines[0].line_number, 1);
    }

    #[test]
    fn r_imp_26_bank_value_and_purchase_dates() {
        let s = parse(&statement(), context()).unwrap();
        let card = &s.lines[1];
        assert_eq!(card.date, Some(d("2025-12-05")));
        assert_eq!(card.value_date, Some(d("2025-12-05")));
        assert_eq!(card.purchase_date, Some(d("2025-12-03")));
        assert_eq!(s.lines[0].purchase_date, None, "seul un paiement par carte a une date d'achat");
        // Relevé à cheval : janvier dans l'année de fin, l'achat de décembre dans l'année précédente (R-IMP-4).
        let last = &s.lines[4];
        assert_eq!(last.date, Some(d("2026-01-02")));
        assert_eq!(last.purchase_date, Some(d("2025-12-31")));
    }

    #[test]
    fn r_imp_2_amounts_dates_and_card_labels() {
        assert!(is_amount("1 234,56") && is_amount("4,20") && is_amount("13 947,74") && is_amount("-0,50"));
        assert!(!is_amount("1234 56,00") && !is_amount("12,5") && !is_amount("EUR 3,72") && !is_amount(",50"));
        assert_eq!(period("  du 03.09.2026 au 02.10.2026 -  N° 218"), Some(("03.09.2026", "02.10.2026")));
        assert_eq!(card_date("CB LIDL 05/09/26"), Some("05/09/26"));
        assert_eq!(card_date("PRLV SEPA LIDL 05/09/26"), None);
        assert!(is_footer("Page 2 / 5") && !is_footer("Pages jaunes"));
    }

    #[test]
    fn r_imp_23_other_documents_are_refused() {
        let other = vec![item(1, 700.0, 40.0, "FACTURE"), item(1, 680.0, 40.0, "du 01.01.2026 au 31.01.2026")];
        assert!(parse(&other, context()).is_err());
        assert!(parse(&[item(1, 700.0, 40.0, "RELEVE DE COMPTE")], context()).is_err());
    }

    #[test]
    fn r_imp_3_a_line_without_amount_is_kept_with_an_error() {
        let mut items = statement();
        items.extend(op(3, 599.0, "01.01", "OPERATION SANS MONTANT", "01.01.26", None, None));
        let s = parse(&items, context()).unwrap();
        let line = s.lines.iter().find(|l| l.bank_label == "OPERATION SANS MONTANT").unwrap();
        assert!(!line.is_valid());
        assert_eq!(line.errors, ["montant absent"]);
    }

    /// Golden file : un relevé fictif à la mise en page LCL, lu de bout en bout (extraction comprise).
    #[test]
    fn imp_10_reads_the_fictitious_lcl_statement() {
        let content =
            std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/lcl/releve-fictif.pdf")).unwrap();
        let s = LclPdfReader.read(&content, context()).unwrap();
        assert_eq!(s.period, Some((d("2025-12-03"), d("2026-01-02"))));
        assert_eq!(
            (s.opening_balance, s.closing_balance),
            (Some(Money::from_minor(125_000)), Some(Money::from_minor(251_995)))
        );
        assert_eq!(s.lines.len(), 7);
        assert!(s.lines.iter().all(|l| l.errors.is_empty()));
        let total: Money = s.lines.iter().filter_map(|l| l.amount).sum();
        assert_eq!(s.opening_balance.unwrap() + total, s.closing_balance.unwrap());
        assert_eq!(s.lines[0].bank_label, "VIREMENT EMPLOYEUR FICTIF\nLIBELLE:SALAIRE NOVEMBRE\nREF.CLIENT:000000000");
        assert_eq!(s.lines[2].amount, Some(Money::from_minor(-1_250)));
        assert_eq!(s.lines[6].purchase_date, Some(d("2025-12-31")));
        assert_eq!(s.lines[6].date, Some(d("2026-01-02")));
    }

    /// Vérification locale sur un vrai relevé, jamais versionné (invariant 7) :
    /// `CASHFLOW_LCL_PDF=/chemin/releve.pdf cargo test -p cashflow-server real_lcl_statement -- --ignored`.
    #[test]
    #[ignore = "demande un relevé LCL réel, hors du dépôt"]
    fn real_lcl_statement() {
        let path = std::env::var("CASHFLOW_LCL_PDF").expect("CASHFLOW_LCL_PDF");
        let content = std::fs::read(path).unwrap();
        let s = LclPdfReader.read(&content, context()).unwrap();
        let total: Money = s.lines.iter().filter_map(|l| l.amount).sum();
        let errors: Vec<_> =
            s.lines.iter().filter(|l| !l.errors.is_empty()).map(|l| (l.line_number, &l.errors)).collect();
        let cards = s.lines.iter().filter(|l| l.purchase_date.is_some()).count();
        println!("{} opérations, {cards} paiements par carte, erreurs : {errors:?}", s.lines.len());
        println!("période {:?}, solde de fin au {:?}", s.period, s.closing_date);
        assert!(errors.is_empty());
        assert_eq!(s.opening_balance.unwrap() + total, s.closing_balance.unwrap(), "contrôle du relevé (R-IMP-18)");
    }
}
