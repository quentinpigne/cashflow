from datetime import date
from sqlalchemy import select
from sqlalchemy.orm import Session

from app.models.transaction import Transaction
from app.repositories.base import BaseRepository


class TransactionRepository(BaseRepository):
    def __init__(self, db: Session):
        super().__init__(db, Transaction)

    def get_all(
        self,
        start_date: date | None = None,
        end_date: date | None = None,
        offset: int | None = None,
        limit: int | None = None,
    ) -> list[Transaction]:
        return self.db.scalars(
            select(self.model_class)
            .where(
                *filter(
                    lambda x: x is not None,
                    [
                        Transaction.date >= start_date if start_date else None,
                        Transaction.date <= end_date if end_date else None,
                    ],
                )
            )
            .offset(offset)
            .limit(limit)
        ).all()

    def get_by_account_id(
        self,
        account_id: int,
        start_date: date | None = None,
        end_date: date | None = None,
        skip: int = 0,
        limit: int = 100,
    ) -> list[Transaction]:
        return self.db.scalars(
            select(Transaction)
            .where(
                *filter(
                    lambda x: x is not None,
                    [
                        Transaction.account_id == account_id,
                        Transaction.date >= start_date if start_date else None,
                        Transaction.date <= end_date if end_date else None,
                    ],
                )
            )
            .order_by(Transaction.date.desc())
            .offset(skip)
            .limit(limit)
        ).all()
