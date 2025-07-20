from sqlalchemy import Column, ForeignKey, Integer, String
from sqlalchemy.orm import relationship

from app.core.database import Base


class Category(Base):
    __tablename__ = "transaction_category"

    id = Column(Integer, primary_key=True, autoincrement=True)
    name = Column(String(50), nullable=False)
    description = Column(String(200), nullable=True)
    type = Column(String(50), nullable=False, default="expense")
    parent_id = Column(Integer, ForeignKey("transaction_category.id"), nullable=True)

    parent = relationship("Category", remote_side=[id], back_populates="children")
    children = relationship("Category", back_populates="parent")
    transactions = relationship("Transaction", back_populates="category")
