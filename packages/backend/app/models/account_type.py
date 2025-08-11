from typing import TYPE_CHECKING

from sqlalchemy import String
from sqlalchemy.orm import Mapped, relationship, mapped_column

from app.core.database import Base

if TYPE_CHECKING:
    from app.models import Account


class AccountType(Base):
    __tablename__ = "account_type"

    id: Mapped[int] = mapped_column(primary_key=True, autoincrement=True)
    name: Mapped[str] = mapped_column(String(50), index=True)

    accounts: Mapped["Account"] = relationship(back_populates="account_type")
