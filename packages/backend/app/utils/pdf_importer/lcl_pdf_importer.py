from functools import reduce
import io
import re
import pandas as pd

from datetime import datetime
from typing import NamedTuple

from app.utils.pdf_importer.pdf_importer import PdfImporter


class LclPdfImporter(PdfImporter):
    def import_transactions_from_pdf(self) -> pd.DataFrame:
        self.start_date = datetime.strptime(
            re.search(
                r"du (\d+\.\d+\.\d+) au \d+\.\d+\.\d+", self.pages[0].markdown
            ).group(1),
            "%d.%m.%Y",
        )

        for page in self.pages:
            for table in page.tables:
                table_content = table.content
                if "ECRITURES DE LA PERIODE" in table_content:
                    self._pages_dataframes.append(self.__process_table(table_content))
        return pd.concat(self._pages_dataframes)

    def __process_table(self, table_content: str) -> pd.DataFrame:
        table_dataframe = self._filter_dataframe(
            pd.read_html(
                io.StringIO(table_content),
                skiprows=1,
                header=0,
                thousands=" ",
                decimal=",",
            )[0],
            "LIBELLE",
            [
                "ANCIEN SOLDE",
                "TOTAUX",
                "SOLDE EN EUROS",
                "SOLDE INTERMEDIAIRE A .*",
            ],
        )
        return pd.DataFrame(
            reduce(self.__process_row, table_dataframe.itertuples(index=False), []),
            columns=["DATE", "LIBELLE", "MONTANT"],
        )

    def __process_row(self, rows, current_row: NamedTuple):
        return (
            [
                *rows[:-1],
                [rows[-1][0], rows[-1][1] + "\n" + current_row.LIBELLE, rows[-1][2]],
            ]
            if pd.isna(current_row.DATE)
            else [
                *rows,
                [
                    self.__process_date(current_row),
                    current_row.LIBELLE,
                    self.__process_amount(current_row),
                ],
            ]
        )

    def __process_date(self, row: NamedTuple) -> str:
        match = re.search(r"CB .* (\d+/\d+/\d+)", row.LIBELLE)
        if match:
            return datetime.strptime(match.group(1), "%d/%m/%y").strftime("%d/%m/%Y")
        else:
            [day, month] = map(int, str(row.DATE).split("."))
            return f"{day:02}/{month:02}/{
                self.start_date.year + 1
                if month == 1 and self.start_date.month == 12
                else self.start_date.year
            }"

    def __process_amount(self, row: NamedTuple) -> float:
        return float(row.CREDIT) if pd.isna(row.DEBIT) else -float(row.DEBIT)
