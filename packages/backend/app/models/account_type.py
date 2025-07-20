from sqlalchemy import Column, Integer, String
from sqlalchemy.orm import relationship

from app.core.database import Base


class AccountType(Base):
    __tablename__ = "account_type"

    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, index=True)

    accounts = relationship("Account", back_populates="account_type")
