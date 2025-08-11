from pydantic import BaseModel, ConfigDict

from app.schemas.account_type import AccountType
from app.schemas.bank import Bank
from app.schemas.transaction import Transaction


class AccountBase(BaseModel):
    name: str
    description: str | None
    agency_code: str
    account_number: str
    rib_key: str
    iban: str
    bic: str
    currency: str
    initial_balance: float


class AccountCreate(AccountBase):
    bank_id: int
    account_type_id: int


class AccountUpdate(AccountBase):
    bank_id: int
    account_type_id: int


class Account(AccountBase):
    model_config = ConfigDict(from_attributes=True)

    id: int
    current_balance: float
    bank: Bank
    account_type: AccountType
    transactions: list[Transaction] = []
