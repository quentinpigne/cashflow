import datetime

from typing import TYPE_CHECKING

from sqlalchemy import ForeignKey, String
from sqlalchemy.orm import Mapped, relationship, mapped_column

from app.core.database import Base

if TYPE_CHECKING:
    from app.models.account import Account
    from app.models.category import Category


class Transaction(Base):
    __tablename__ = "transaction"

    id: Mapped[int] = mapped_column(primary_key=True, autoincrement=True)
    bank_label: Mapped[str] = mapped_column(String(250))
    custom_label: Mapped[str | None] = mapped_column(String(250))
    comment: Mapped[str | None] = mapped_column(String(250))
    amount: Mapped[float] = mapped_column()
    date: Mapped[datetime.datetime] = mapped_column()
    account_id: Mapped[int] = mapped_column(ForeignKey("account.id"))
    category_id: Mapped[int | None] = mapped_column(
        ForeignKey("transaction_category.id")
    )

    account: Mapped["Account"] = relationship(back_populates="transactions")
    category: Mapped["Category"] = relationship(back_populates="transactions")
