from pydantic import BaseModel


class AccountType(BaseModel):
    id: int
    name: str
