from pydantic import BaseModel, ConfigDict


class BankBase(BaseModel):
    name: str
    bank_code: str | None = None


class BankCreate(BankBase):
    pass


class BankUpdate(BankBase):
    pass


class Bank(BankBase):
    model_config = ConfigDict(from_attributes=True)

    id: int
