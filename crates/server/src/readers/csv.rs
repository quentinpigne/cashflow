//! Lecture des fichiers CSV : encodage et séparateur détectés, modifiables par le profil (IMP-01).

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};

use super::{Cell, ReadError, Table};

const DELIMITERS: [char; 4] = [';', ',', '\t', '|'];

/// Encodage probable : UTF-8 s'il est valide (avec ou sans BOM), sinon windows-1252.
pub fn detect_encoding(content: &[u8]) -> &'static str {
    if content.starts_with(b"\xEF\xBB\xBF") {
        "UTF-8-BOM"
    } else if std::str::from_utf8(content).is_ok() {
        "UTF-8"
    } else {
        "windows-1252"
    }
}

pub fn decode(content: &[u8], encoding: &str) -> Result<String, ReadError> {
    let encoding: &'static Encoding = match encoding {
        "UTF-8" | "UTF-8-BOM" | "utf-8" => UTF_8,
        // ISO-8859-1 est un sous-ensemble de windows-1252 pour les caractères imprimables.
        "windows-1252" | "ISO-8859-1" | "latin1" => WINDOWS_1252,
        other => Encoding::for_label(other.as_bytes())
            .ok_or_else(|| ReadError::Settings(format!("encodage inconnu : {other}")))?,
    };
    let (text, _, _) = encoding.decode(content);
    Ok(text.trim_start_matches('\u{feff}').to_owned())
}

/// Séparateur le plus régulier sur les premières lignes.
pub fn detect_delimiter(text: &str) -> char {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(30).collect();
    DELIMITERS
        .into_iter()
        .map(|d| {
            let counts: Vec<usize> = lines.iter().map(|l| l.matches(d).count()).collect();
            let max = counts.iter().copied().max().unwrap_or(0);
            // Nombre de lignes qui ont le nombre de colonnes le plus fréquent.
            let regular = counts.iter().filter(|&&c| c == max && c > 0).count();
            (d, regular * 1000 + max)
        })
        .max_by_key(|(_, score)| *score)
        .map_or(';', |(d, _)| d)
}

/// Découpe un texte CSV en tableau de cellules texte.
pub fn table(text: &str, delimiter: char, quote: char) -> Result<Table, ReadError> {
    let delimiter = u8::try_from(delimiter).map_err(|_| ReadError::Settings("séparateur non ASCII".into()))?;
    let quote = u8::try_from(quote).map_err(|_| ReadError::Settings("guillemet non ASCII".into()))?;
    let mut reader = ::csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .quote(quote)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    reader
        .records()
        .map(|record| {
            record
                .map(|r| r.iter().map(|c| Cell::Text(c.trim().to_owned())).collect())
                .map_err(|e| ReadError::Unreadable(e.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_windows_1252_and_semicolon() {
        let (bytes, _, _) = WINDOWS_1252.encode("Date;Libellé;Débit;Crédit\n12/03/2026;CAFÉ;1 234,56;\n");
        assert_eq!(detect_encoding(&bytes), "windows-1252");
        let text = decode(&bytes, "windows-1252").unwrap();
        assert!(text.contains("Libellé"));
        assert_eq!(detect_delimiter(&text), ';');
        let table = table(&text, ';', '"').unwrap();
        assert_eq!(table[1][2], Cell::Text("1 234,56".into()));
    }

    #[test]
    fn detects_comma_and_strips_bom() {
        let text = decode(b"\xEF\xBB\xBFa,b,c\n1,2,3\n", "UTF-8-BOM").unwrap();
        assert!(text.starts_with('a'));
        assert_eq!(detect_delimiter(&text), ',');
    }
}
