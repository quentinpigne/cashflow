from sqlalchemy import Column, Date, Float, ForeignKey, Integer, String
from sqlalchemy.orm import relationship

from app.core.database import Base


class Transaction(Base):
    __tablename__ = "transaction"

    id = Column(Integer, primary_key=True, autoincrement=True)
    bank_label = Column(String(50), nullable=False)
    custom_label = Column(String(50), nullable=True)
    comment = Column(String(200), nullable=True)
    amount = Column(Float, nullable=False)
    date = Column(Date, nullable=False)
    account_id = Column(Integer, ForeignKey("account.id"), nullable=False)
    category_id = Column(Integer, ForeignKey("transaction_category.id"), nullable=True)

    account = relationship("Account", back_populates="transactions")
    category = relationship("Category", back_populates="transactions")
