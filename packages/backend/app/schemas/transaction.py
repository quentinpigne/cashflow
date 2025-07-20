from datetime import date
from typing import Optional

from pydantic import BaseModel, ConfigDict


class TransactionBase(BaseModel):
    bank_label: str
    custom_label: Optional[str] = None
    comment: Optional[str] = None
    type: str
    amount: float
    date: date
    account_id: int
    category_id: Optional[int] = None


class TransactionCreate(TransactionBase):
    pass


class TransactionUpdate(TransactionBase):
    pass


class Transaction(TransactionBase):
    model_config = ConfigDict(from_attributes=True)

    id: int
