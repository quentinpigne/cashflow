from sqlalchemy import Column, Integer, String
from sqlalchemy.orm import relationship

from app.core.database import Base


class Bank(Base):
    __tablename__ = "bank"

    id = Column(Integer, primary_key=True)
    name = Column(String(50), nullable=False)
    bank_code = Column(String(5))

    accounts = relationship("Account", back_populates="bank")
