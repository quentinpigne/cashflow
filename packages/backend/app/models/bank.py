from typing import TYPE_CHECKING

from sqlalchemy import String
from sqlalchemy.orm import Mapped, relationship, mapped_column

from app.core.database import Base

if TYPE_CHECKING:
    from app.models.account import Account


class Bank(Base):
    __tablename__ = "bank"

    id: Mapped[int] = mapped_column(primary_key=True, autoincrement=True)
    name: Mapped[str] = mapped_column(String(50))
    bank_code: Mapped[str | None] = mapped_column(String(5))

    accounts: Mapped[list["Account"]] = relationship(back_populates="bank")
