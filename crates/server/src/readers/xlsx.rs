//! Lecture des classeurs XLSX (IMP-01) : une feuille, cellules typées.

use std::io::Cursor;

use calamine::{Data, Reader, open_workbook_auto_from_rs};

use super::{Cell, ReadError, Table};

pub fn sheet_names(content: &[u8]) -> Result<Vec<String>, ReadError> {
    let workbook =
        open_workbook_auto_from_rs(Cursor::new(content)).map_err(|e| ReadError::Unreadable(e.to_string()))?;
    Ok(workbook.sheet_names())
}

/// Tableau d'une feuille (la première si `sheet` est absent).
pub fn table(content: &[u8], sheet: Option<&str>) -> Result<Table, ReadError> {
    let mut workbook =
        open_workbook_auto_from_rs(Cursor::new(content)).map_err(|e| ReadError::Unreadable(e.to_string()))?;
    let name = match sheet {
        Some(name) => name.to_owned(),
        None => workbook.sheet_names().first().cloned().ok_or_else(|| ReadError::Unreadable("classeur vide".into()))?,
    };
    let range =
        workbook.worksheet_range(&name).map_err(|e| ReadError::Settings(format!("feuille « {name} » : {e}")))?;
    Ok(range.rows().map(|row| row.iter().map(cell).collect()).collect())
}

fn cell(data: &Data) -> Cell {
    match data {
        Data::Empty | Data::Error(_) => Cell::Text(String::new()),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Text(s.trim().to_owned()),
        Data::Bool(b) => Cell::Text(b.to_string()),
        Data::Int(i) => Cell::Number(i.to_string()),
        // Le tableur stocke ses nombres en flottant binaire : on passe par leur écriture décimale la
        // plus courte, que le profil analyse ensuite comme un texte (R-IMP-2).
        Data::Float(f) => Cell::Number(format!("{f}")),
        Data::DateTime(dt) => match dt.as_datetime() {
            Some(datetime) => Cell::Date(datetime.date()),
            None => Cell::Text(dt.to_string()),
        },
    }
}
