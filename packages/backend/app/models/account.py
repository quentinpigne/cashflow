from typing import TYPE_CHECKING

from sqlalchemy import ForeignKey, String
from sqlalchemy.orm import Mapped, relationship, mapped_column

from app.core.database import Base

if TYPE_CHECKING:
    from app.models.bank import Bank
    from app.models.transaction import Transaction
    from app.models.account_type import AccountType


class Account(Base):
    __tablename__ = "account"

    id: Mapped[int] = mapped_column(primary_key=True, autoincrement=True)
    name: Mapped[str] = mapped_column(String(50))
    description: Mapped[str | None] = mapped_column(String(200))
    agency_code: Mapped[str] = mapped_column(String(5))
    account_number: Mapped[str] = mapped_column(String(50))
    rib_key: Mapped[str] = mapped_column(String(2))
    iban: Mapped[str] = mapped_column(String(34))
    bic: Mapped[str] = mapped_column(String(11))
    currency: Mapped[str] = mapped_column(String(3), default="EUR")
    initial_balance: Mapped[float] = mapped_column(default=0)
    current_balance: Mapped[float] = mapped_column(default=0)
    bank_id: Mapped[int] = mapped_column(ForeignKey("bank.id"))
    account_type_id: Mapped[int] = mapped_column(ForeignKey("account_type.id"))

    bank: Mapped["Bank"] = relationship(back_populates="accounts")
    account_type: Mapped["AccountType"] = relationship(back_populates="accounts")
    transactions: Mapped[list["Transaction"]] = relationship(
        back_populates="account", lazy="select", order_by="desc(Transaction.date)"
    )
