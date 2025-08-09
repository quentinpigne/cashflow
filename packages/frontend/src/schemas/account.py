from pydantic import BaseModel

from src.schemas.account_type import AccountType
from src.schemas.bank import Bank
from src.schemas.transaction import Transaction


class Account(BaseModel):
    id: int
    name: str
    description: str
    agency_code: str
    account_number: str
    rib_key: str
    iban: str
    bic: str
    currency: str
    initial_balance: float
    current_balance: float
    bank: Bank
    account_type: AccountType
    transactions: list[Transaction] = []
