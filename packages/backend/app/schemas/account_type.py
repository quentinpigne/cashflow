from pydantic import BaseModel, ConfigDict


class AccountTypeBase(BaseModel):
    name: str


class AccountTypeCreate(AccountTypeBase):
    pass


class AccountType(AccountTypeBase):
    model_config = ConfigDict(from_attributes=True)
    id: int
