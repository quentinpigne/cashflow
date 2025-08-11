from typing import TYPE_CHECKING

from sqlalchemy import ForeignKey, String
from sqlalchemy.orm import Mapped, relationship, mapped_column

from app.core.database import Base

if TYPE_CHECKING:
    from app.models.category import Category
    from app.models.transaction import Transaction


class Category(Base):
    __tablename__ = "transaction_category"

    id: Mapped[int] = mapped_column(primary_key=True, autoincrement=True)
    name: Mapped[str] = mapped_column(String(50))
    description: Mapped[str | None] = mapped_column(String(200))
    type: Mapped[str] = mapped_column(String(50), default="expense")
    parent_id: Mapped[int | None] = mapped_column(ForeignKey("transaction_category.id"))

    parent: Mapped["Category"] = relationship(
        remote_side=[id], back_populates="children"
    )
    children: Mapped[list["Category"]] = relationship(back_populates="parent")
    transactions: Mapped[list["Transaction"]] = relationship(back_populates="category")
