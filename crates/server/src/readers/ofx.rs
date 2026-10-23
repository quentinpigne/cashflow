//! Lecture des fichiers OFX / QFX (IMP-09) : SGML 1.x ou XML 2.x, sans profil.

use cashflow_domain::Date;
use cashflow_domain::import::amount::{AmountFormat, parse_amount};
use chrono::NaiveDate;

use super::{ReadContext, ReadError, ReadLine, Statement, StatementReader};

pub struct OfxReader;

/// Valeur d'une balise, fermée (XML) ou non (SGML) : le texte jusqu'au prochain `<`.
fn tag_value<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = block.find(&open)? + open.len();
    let rest = &block[start..];
    let end = rest.find('<').unwrap_or(rest.len());
    Some(rest[..end].trim()).filter(|v| !v.is_empty())
}

/// Blocs `<TAG>…</TAG>`, ou jusqu'au prochain bloc de même nom en SGML.
fn blocks<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut result = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        let end = after.find(&close).or_else(|| after.find(&open)).unwrap_or(after.len());
        result.push(&after[..end]);
        rest = &after[end..];
    }
    result
}

/// Date OFX `AAAAMMJJ[HHMMSS[.XXX]][TZ]`.
fn ofx_date(value: &str) -> Option<Date> {
    let digits: String = value.chars().take(8).collect();
    NaiveDate::parse_from_str(&digits, "%Y%m%d").ok()
}

fn amount(value: &str, context: ReadContext) -> Result<cashflow_domain::Money, String> {
    let format = if value.contains(',') && !value.contains('.') {
        AmountFormat { decimal: ',', thousands: None }
    } else {
        AmountFormat { decimal: '.', thousands: None }
    };
    parse_amount(value, format, context.currency).map_err(|e| e.to_string())
}

impl StatementReader for OfxReader {
    fn read(&self, content: &[u8], context: ReadContext) -> Result<Statement, ReadError> {
        let raw = String::from_utf8_lossy(content);
        let text = if raw.contains("CHARSET:1252") || std::str::from_utf8(content).is_err() {
            encoding_rs::WINDOWS_1252.decode(content).0.into_owned()
        } else {
            raw.into_owned()
        };
        if !text.to_uppercase().contains("<OFX>") {
            return Err(ReadError::Unreadable("balise <OFX> absente".into()));
        }
        let mut statement = Statement::default();
        for (index, block) in blocks(&text, "STMTTRN").into_iter().enumerate() {
            let mut errors = Vec::new();
            let date = tag_value(block, "DTPOSTED").and_then(ofx_date);
            if date.is_none() {
                errors.push("date illisible".to_owned());
            }
            let value_date = tag_value(block, "DTUSER").and_then(ofx_date);
            let amount = match tag_value(block, "TRNAMT").map(|v| amount(v, context)) {
                Some(Ok(a)) if !a.is_zero() => Some(a),
                Some(Err(e)) => {
                    errors.push(e);
                    None
                }
                _ => {
                    errors.push("montant absent ou nul".to_owned());
                    None
                }
            };
            let name = tag_value(block, "NAME").unwrap_or_default();
            let memo = tag_value(block, "MEMO").unwrap_or_default();
            let bank_label = if memo.is_empty() || name.contains(memo) {
                name.to_owned()
            } else if name.is_empty() {
                memo.to_owned()
            } else {
                format!("{name} {memo}")
            };
            if bank_label.is_empty() {
                errors.push("libellé vide".to_owned());
            }
            let fitid = tag_value(block, "FITID").map(str::to_owned);
            statement.lines.push(ReadLine {
                line_number: i64::try_from(index + 1).unwrap_or(i64::MAX),
                raw: [
                    ("DTPOSTED", tag_value(block, "DTPOSTED")),
                    ("TRNAMT", tag_value(block, "TRNAMT")),
                    ("FITID", fitid.as_deref()),
                    ("NAME", Some(name)),
                    ("MEMO", Some(memo)),
                ]
                .into_iter()
                .map(|(k, v)| format!("{k}={}", v.unwrap_or_default()))
                .collect(),
                date,
                value_date,
                amount,
                bank_label,
                fitid,
                bank_category: None,
                purchase_date: None,
                errors,
            });
        }
        if let Some(list) = blocks(&text, "BANKTRANLIST").first() {
            statement.period =
                tag_value(list, "DTSTART").and_then(ofx_date).zip(tag_value(list, "DTEND").and_then(ofx_date));
        }
        if let Some(ledger) = blocks(&text, "LEDGERBAL").first() {
            statement.closing_balance = tag_value(ledger, "BALAMT").and_then(|v| amount(v, context).ok());
            statement.closing_date = tag_value(ledger, "DTASOF").and_then(ofx_date);
        }
        statement.fill_period();
        Ok(statement)
    }
}

#[cfg(test)]
mod tests {
    use cashflow_domain::{Currency, Money};

    use super::*;

    const SGML: &str = "OFXHEADER:100\nDATA:OFXSGML\nCHARSET:1252\n\n<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><CURDEF>EUR\n<BANKTRANLIST><DTSTART>20260301<DTEND>20260331\n<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260310120000<TRNAMT>-4.20<FITID>ABC123<NAME>CB BOULANGERIE\n<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260327<TRNAMT>2850,00<FITID>ABC124<NAME>VIR SALAIRE<MEMO>MARS\n</BANKTRANLIST><LEDGERBAL><BALAMT>1520.35<DTASOF>20260331</LEDGERBAL></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>";

    #[test]
    fn reads_sgml_ofx() {
        let context = ReadContext { today: "2026-10-20".parse().unwrap(), currency: Currency::EUR };
        let statement = OfxReader.read(SGML.as_bytes(), context).unwrap();
        assert_eq!(statement.lines.len(), 2);
        assert_eq!(statement.lines[0].amount, Some(Money::from_minor(-420)));
        assert_eq!(statement.lines[0].fitid.as_deref(), Some("ABC123"));
        assert_eq!(statement.lines[1].bank_label, "VIR SALAIRE MARS");
        assert_eq!(statement.lines[1].amount, Some(Money::from_minor(285_000)));
        assert_eq!(statement.closing_balance, Some(Money::from_minor(152_035)));
        assert_eq!(statement.closing_date, Some("2026-03-31".parse().unwrap()));
        assert_eq!(statement.period, Some(("2026-03-01".parse().unwrap(), "2026-03-31".parse().unwrap())));
    }
}
