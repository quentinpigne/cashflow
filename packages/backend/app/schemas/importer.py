from typing import Dict, Optional, Any, List

from pydantic import BaseModel


class ColumnMapping(BaseModel):
    column: str
    transformation: Optional[Dict[str, Any]] = None


class ImportConfig(BaseModel):
    bank: Optional[str] = None
    delimiter: Optional[str] = None
    sheet_name: Optional[str] = None
    rows_to_skip: Optional[List[int]] = None
    column_mapping: Dict[str, str | ColumnMapping]
