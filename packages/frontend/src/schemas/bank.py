from pydantic import BaseModel


class Bank(BaseModel):
    id: int
    name: str
    bank_code: str | None = None
