from sqlalchemy import select
from sqlalchemy.orm import Session

from app.models.transaction import Transaction
from app.repositories.base import BaseRepository


class TransactionRepository(BaseRepository):
    def __init__(self, db: Session):
        super().__init__(db, Transaction)

    def get_by_account_id(
        self, account_id: int, skip: int = 0, limit: int = 100
    ) -> list[Transaction]:
        return self.db.scalars(
            select(Transaction)
            .where(Transaction.account_id == account_id)
            .order_by(Transaction.date.desc())
            .offset(skip)
            .limit(limit)
        ).all()
