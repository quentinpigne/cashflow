from datetime import date
from typing import Optional

from pydantic import BaseModel


class Transaction(BaseModel):
    id: int
    bank_label: str
    custom_label: Optional[str] = None
    comment: Optional[str] = None
    amount: float
    date: date
    account_id: int
    category_id: Optional[int] = None
