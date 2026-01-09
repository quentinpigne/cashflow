import base64
import pandas as pd

from typing import List
from abc import ABC, abstractmethod

from mistralai import Mistral, OCRPageObject

from app.core.config import settings


class PdfImporter(ABC):
    def __init__(self, content: bytes):
        self._pages_dataframes = []
        self.pages = self.__read_pdf_using_ocr(content)

    def __read_pdf_using_ocr(self, content: bytes) -> List[OCRPageObject]:
        mistral = Mistral(api_key=settings.MISTRAL_APIKEY)
        return mistral.ocr.process(
            model="mistral-ocr-latest",
            document={
                "type": "document_url",
                "document_url": f"data:application/pdf;base64,{base64.b64encode(content).decode('utf-8')}",
            },
            table_format="html",
        ).pages

    def _filter_dataframe(self, df: pd.DataFrame, column: str, filters: List[str]):
        return df[~df[column].str.contains("|".join(filters))].copy()

    @abstractmethod
    def import_transactions_from_pdf(self) -> pd.DataFrame:
        pass
