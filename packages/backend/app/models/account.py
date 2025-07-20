from sqlalchemy import Column, ForeignKey, Integer, String
from sqlalchemy.orm import relationship

from app.core.database import Base


class Account(Base):
    __tablename__ = "account"

    id = Column(Integer, primary_key=True, autoincrement=True)
    name = Column(String(50), nullable=False)
    description = Column(String(200))
    agency_code = Column(String(5), nullable=False)
    account_number = Column(String(50), nullable=False)
    rib_key = Column(String(2), nullable=False)
    iban = Column(String(34), nullable=False)
    bic = Column(String(11), nullable=False)
    currency = Column(String(3), nullable=False, default="EUR")
    initial_balance = Column(Integer, nullable=False, default=0)
    current_balance = Column(Integer, nullable=False, default=0)
    bank_id = Column(Integer, ForeignKey("bank.id"), nullable=False)
    account_type_id = Column(Integer, ForeignKey("account_type.id"), nullable=False)

    bank = relationship("Bank", back_populates="accounts")
    account_type = relationship("AccountType", back_populates="accounts")
    transactions = relationship("Transaction", back_populates="account")
