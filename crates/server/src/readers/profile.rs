//! Profils d'import CSV/XLSX (spec 03, § « Profil d'import ») : du tableau brut aux lignes normalisées.

use cashflow_domain::Money;
use cashflow_domain::import::amount::{AmountFormat, parse_amount};
use cashflow_domain::import::date::{DateFormat, YearContext, parse_date};
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Cell, ReadContext, ReadError, ReadLine, Statement, Table};

/// Réglages d'un profil (`ImportProfile.settings`), tels que décrits par la spec 03.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ProfileSettings {
    /// CSV : `UTF-8`, `UTF-8-BOM`, `windows-1252`, `ISO-8859-1`.
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub delimiter: Option<String>,
    #[serde(default = "default_quote")]
    pub quote: String,
    /// XLSX : nom de la feuille (la première par défaut).
    #[serde(default)]
    pub sheet: Option<String>,
    /// Ligne des en-têtes, à partir de 1 ; absente : colonnes désignées par leur numéro.
    #[serde(default)]
    pub header_row: Option<usize>,
    #[serde(default)]
    pub skip_rows_after_header: usize,
    #[serde(default)]
    pub skip_footer_rows: usize,
    pub date: DateColumn,
    #[serde(default)]
    pub value_date: Option<DateColumn>,
    pub label: LabelColumns,
    pub amount: AmountColumns,
    #[serde(default)]
    pub row_filters: Vec<RowFilter>,
    #[serde(default)]
    pub statement_balances: Option<StatementBalances>,
    #[serde(default)]
    pub category_column: Option<ColumnRef>,
}

fn default_quote() -> String {
    "\"".into()
}

/// Colonne désignée par son en-tête ou par son numéro (1 = première colonne).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum ColumnRef {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct DateColumn {
    pub column: ColumnRef,
    /// Par exemple `dd/MM/yyyy`, `yyyy-MM-dd`, `dd/MM/yy`.
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LabelColumns {
    pub columns: Vec<ColumnRef>,
    #[serde(default = "default_separator")]
    pub separator: String,
}

fn default_separator() -> String {
    " ".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AmountMode {
    Single,
    DebitCredit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AmountColumns {
    pub mode: AmountMode,
    #[serde(default)]
    pub column: Option<ColumnRef>,
    #[serde(default)]
    pub debit_column: Option<ColumnRef>,
    #[serde(default)]
    pub credit_column: Option<ColumnRef>,
    /// Mode débit/crédit : le débit est-il écrit en positif ?
    #[serde(default = "yes")]
    pub debit_is_positive: bool,
    /// Mode simple : inverser le signe (export de carte, par exemple).
    #[serde(default)]
    pub invert_sign: bool,
    #[serde(default = "default_decimal")]
    pub decimal_separator: String,
    /// Espace, espace insécable, point, apostrophe ; vide : aucun.
    #[serde(default = "default_thousands")]
    pub thousands_separator: String,
}

fn yes() -> bool {
    true
}

fn default_decimal() -> String {
    ",".into()
}

fn default_thousands() -> String {
    " ".into()
}

/// Ligne ignorée si la colonne correspond au motif (soldes, totaux…).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RowFilter {
    pub column: ColumnRef,
    pub pattern: String,
    #[serde(default)]
    pub regex: bool,
}

/// Cellules où lire les soldes du relevé (par exemple `B2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct StatementBalances {
    #[serde(default)]
    pub opening: Option<CellRef>,
    #[serde(default)]
    pub closing: Option<CellRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CellRef {
    pub cell: String,
}

impl AmountColumns {
    fn format(&self) -> AmountFormat {
        AmountFormat {
            decimal: self.decimal_separator.chars().next().unwrap_or(','),
            thousands: self.thousands_separator.chars().next(),
        }
    }
}

/// Position (ligne, colonne) d'une référence de cellule `B2`, à partir de 0.
fn cell_position(reference: &str) -> Option<(usize, usize)> {
    let reference = reference.trim().to_uppercase();
    let letters: String = reference.chars().take_while(char::is_ascii_alphabetic).collect();
    let digits: String = reference.chars().skip(letters.len()).collect();
    if letters.is_empty() || digits.is_empty() {
        return None;
    }
    let column = letters.bytes().fold(0usize, |acc, b| acc * 26 + usize::from(b - b'A' + 1));
    let row: usize = digits.parse().ok()?;
    Some((row.checked_sub(1)?, column.checked_sub(1)?))
}

struct Columns<'a> {
    headers: Option<&'a [Cell]>,
}

impl Columns<'_> {
    fn index(&self, column: &ColumnRef) -> Result<usize, ReadError> {
        match column {
            ColumnRef::Index(n) => {
                n.checked_sub(1).ok_or_else(|| ReadError::Settings("les colonnes se numérotent à partir de 1".into()))
            }
            ColumnRef::Name(name) => {
                let headers = self
                    .headers
                    .ok_or_else(|| ReadError::Settings(format!("colonne « {name} » : pas de ligne d'en-têtes")))?;
                headers
                    .iter()
                    .position(|h| h.text().trim().eq_ignore_ascii_case(name.trim()))
                    .ok_or_else(|| ReadError::Settings(format!("colonne « {name} » introuvable")))
            }
        }
    }
}

fn cell_at(row: &[Cell], index: usize) -> Cell {
    row.get(index).cloned().unwrap_or(Cell::Text(String::new()))
}

/// Montant d'une cellule : nombre du tableur (point décimal) ou texte selon le format du profil.
fn amount_of(cell: &Cell, format: AmountFormat, context: ReadContext) -> Result<Option<Money>, String> {
    match cell {
        Cell::Number(n) => parse_amount(n, AmountFormat { decimal: '.', thousands: None }, context.currency)
            .map(Some)
            .map_err(|e| e.to_string()),
        Cell::Text(t) if t.trim().is_empty() => Ok(None),
        other => parse_amount(&other.text(), format, context.currency).map(Some).map_err(|e| e.to_string()),
    }
}

/// Applique un profil à un tableau : lignes normalisées, erreurs par ligne, soldes du relevé.
pub fn apply(table: &Table, settings: &ProfileSettings, context: ReadContext) -> Result<Statement, ReadError> {
    let header_index = settings.header_row.map(|h| h.saturating_sub(1));
    let headers = header_index.and_then(|h| table.get(h)).map(Vec::as_slice);
    let columns = Columns { headers };
    let date_format = DateFormat::parse(&settings.date.format).map_err(|e| ReadError::Settings(e.to_string()))?;
    let value_date = match &settings.value_date {
        Some(v) => Some((
            columns.index(&v.column)?,
            DateFormat::parse(&v.format).map_err(|e| ReadError::Settings(e.to_string()))?,
        )),
        None => None,
    };
    let date_column = columns.index(&settings.date.column)?;
    let label_columns = settings.label.columns.iter().map(|c| columns.index(c)).collect::<Result<Vec<_>, _>>()?;
    if label_columns.is_empty() {
        return Err(ReadError::Settings("au moins une colonne de libellé".into()));
    }
    let amount_format = settings.amount.format();
    let (single, debit, credit) = match settings.amount.mode {
        AmountMode::Single => {
            let column = settings
                .amount
                .column
                .as_ref()
                .ok_or_else(|| ReadError::Settings("colonne du montant manquante".into()))?;
            (Some(columns.index(column)?), None, None)
        }
        AmountMode::DebitCredit => {
            let debit = settings
                .amount
                .debit_column
                .as_ref()
                .ok_or_else(|| ReadError::Settings("colonne du débit manquante".into()))?;
            let credit = settings
                .amount
                .credit_column
                .as_ref()
                .ok_or_else(|| ReadError::Settings("colonne du crédit manquante".into()))?;
            (None, Some(columns.index(debit)?), Some(columns.index(credit)?))
        }
    };
    let filters = settings
        .row_filters
        .iter()
        .map(|f| {
            let pattern = if f.regex { f.pattern.clone() } else { regex::escape(&f.pattern) };
            let regex = RegexBuilder::new(&pattern)
                .case_insensitive(true)
                .build()
                .map_err(|e| ReadError::Settings(format!("filtre « {} » : {e}", f.pattern)))?;
            Ok((columns.index(&f.column)?, regex))
        })
        .collect::<Result<Vec<_>, ReadError>>()?;
    let category_column = settings.category_column.as_ref().map(|c| columns.index(c)).transpose()?;

    let first = header_index.map_or(0, |h| h + 1) + settings.skip_rows_after_header;
    let last = table.len().saturating_sub(settings.skip_footer_rows);
    let year_context = YearContext { today: context.today, period: None };

    let mut lines = Vec::new();
    for (index, row) in table.iter().enumerate().take(last).skip(first) {
        if row.iter().all(Cell::is_empty) {
            continue;
        }
        if filters.iter().any(|(column, regex)| regex.is_match(&cell_at(row, *column).text())) {
            continue;
        }
        let mut errors = Vec::new();
        let date = match cell_at(row, date_column) {
            Cell::Date(d) => Some(d),
            cell => parse_date(&cell.text(), &date_format, year_context).map_err(|e| errors.push(e.to_string())).ok(),
        };
        let value_date = value_date.as_ref().and_then(|(column, format)| match cell_at(row, *column) {
            Cell::Date(d) => Some(d),
            cell if cell.is_empty() => None,
            cell => parse_date(&cell.text(), format, year_context).ok(),
        });
        let bank_label = label_columns
            .iter()
            .map(|c| cell_at(row, *c).text().trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(&settings.label.separator);
        if bank_label.is_empty() {
            errors.push("libellé vide".into());
        }
        let amount = match (single, debit, credit) {
            (Some(column), _, _) => amount_of(&cell_at(row, column), amount_format, context)
                .map(|a| a.map(|m| if settings.amount.invert_sign { -m } else { m })),
            (_, Some(debit), Some(credit)) => {
                let debit = amount_of(&cell_at(row, debit), amount_format, context);
                let credit = amount_of(&cell_at(row, credit), amount_format, context);
                match (debit, credit) {
                    (Err(e), _) | (_, Err(e)) => Err(e),
                    (Ok(Some(d)), _) if !d.is_zero() => {
                        Ok(Some(if settings.amount.debit_is_positive { -d.abs() } else { d }))
                    }
                    (_, Ok(Some(c))) => Ok(Some(c)),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        };
        let amount = match amount {
            Ok(Some(a)) if !a.is_zero() => Some(a),
            Ok(_) => {
                errors.push("montant absent ou nul".into());
                None
            }
            Err(e) => {
                errors.push(e);
                None
            }
        };
        lines.push(ReadLine {
            line_number: i64::try_from(index + 1).unwrap_or(i64::MAX),
            raw: row.iter().map(Cell::text).collect(),
            date,
            value_date,
            amount,
            bank_label,
            fitid: None,
            bank_category: category_column.map(|c| cell_at(row, c).text()).filter(|t| !t.trim().is_empty()),
            errors,
        });
    }

    let balance = |reference: &Option<CellRef>| -> Option<Money> {
        let (row, column) = cell_position(&reference.as_ref()?.cell)?;
        amount_of(&cell_at(table.get(row)?, column), amount_format, context).ok().flatten()
    };
    let (opening_balance, closing_balance) = match &settings.statement_balances {
        Some(b) => (balance(&b.opening), balance(&b.closing)),
        None => (None, None),
    };
    let mut statement = Statement { lines, period: None, opening_balance, closing_balance, closing_date: None };
    statement.fill_period();
    Ok(statement)
}

#[cfg(test)]
mod tests {
    use cashflow_domain::Currency;

    use super::*;
    use crate::readers::csv;

    fn context() -> ReadContext {
        ReadContext { today: "2026-10-20".parse().unwrap(), currency: Currency::EUR }
    }

    fn settings(json: serde_json::Value) -> ProfileSettings {
        serde_json::from_value(json).unwrap()
    }

    /// Critère d'acceptation d'IMP-01 : windows-1252, `;`, colonnes Débit / Crédit.
    #[test]
    fn imp_01_debit_credit_windows_1252() {
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(
            "Relevé LCL\nSolde initial;2 000,00\nDate;Libellé;Débit;Crédit\n12/03/2026;CB CAFÉ;1 234,56;\n13/03/2026;VIR SALAIRE;;2 850,00\n14/03/2026;TOTAUX;1 234,56;2 850,00\n",
        );
        let text = csv::decode(&bytes, csv::detect_encoding(&bytes)).unwrap();
        let table = csv::table(&text, csv::detect_delimiter(&text), '"').unwrap();
        let s = settings(serde_json::json!({
            "header_row": 3,
            "date": { "column": "Date", "format": "dd/MM/yyyy" },
            "label": { "columns": ["Libellé"] },
            "amount": { "mode": "debit_credit", "debit_column": "Débit", "credit_column": "Crédit" },
            "row_filters": [{ "column": "Libellé", "pattern": "^TOTAUX", "regex": true }],
            "statement_balances": { "opening": { "cell": "B2" } }
        }));
        let statement = apply(&table, &s, context()).unwrap();
        assert_eq!(statement.lines.len(), 2);
        assert_eq!(statement.lines[0].amount, Some(Money::from_minor(-123_456)));
        assert_eq!(statement.lines[0].bank_label, "CB CAFÉ");
        assert_eq!(statement.lines[1].amount, Some(Money::from_minor(285_000)));
        assert_eq!(statement.opening_balance, Some(Money::from_minor(200_000)));
        assert_eq!(statement.period, Some(("2026-03-12".parse().unwrap(), "2026-03-13".parse().unwrap())));
    }

    #[test]
    fn r_imp_3_bad_lines_keep_their_errors() {
        let table: Table = vec![
            vec![Cell::Text("Date".into()), Cell::Text("Libellé".into()), Cell::Text("Montant".into())],
            vec![Cell::Text("31/02/2026".into()), Cell::Text("X".into()), Cell::Text("abc".into())],
            vec![Cell::Text("01/03/2026".into()), Cell::Text("".into()), Cell::Text("-5".into())],
            vec![Cell::Text("02/03/2026".into()), Cell::Text("OK".into()), Cell::Text("-5,1".into())],
        ];
        let s = settings(serde_json::json!({
            "header_row": 1,
            "date": { "column": "Date", "format": "dd/MM/yyyy" },
            "label": { "columns": ["Libellé"] },
            "amount": { "mode": "single", "column": "Montant" }
        }));
        let statement = apply(&table, &s, context()).unwrap();
        assert_eq!(statement.lines.len(), 3);
        assert_eq!(statement.lines[0].errors.len(), 2);
        assert_eq!(statement.lines[1].errors, vec!["libellé vide".to_owned()]);
        assert!(statement.lines[2].is_valid());
        assert_eq!(statement.lines[2].amount, Some(Money::from_minor(-510)));
    }

    #[test]
    fn spreadsheet_cells_and_column_numbers() {
        let table: Table = vec![vec![
            Cell::Date("2026-03-12".parse().unwrap()),
            Cell::Text("CARTE".into()),
            Cell::Number("-12.5".into()),
        ]];
        let s = settings(serde_json::json!({
            "date": { "column": 1, "format": "dd/MM/yyyy" },
            "label": { "columns": [2] },
            "amount": { "mode": "single", "column": 3, "invert_sign": true }
        }));
        let statement = apply(&table, &s, context()).unwrap();
        assert_eq!(statement.lines[0].amount, Some(Money::from_minor(1_250)));
        assert_eq!(cell_position("AB12"), Some((11, 27)));
    }
}
