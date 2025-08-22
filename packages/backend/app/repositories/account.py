from sqlalchemy import desc, func, select
from sqlalchemy.orm import Session, selectinload

from app.models.account import Account
from app.models.transaction import Transaction
from app.repositories.base import BaseRepository


class AccountRepository(BaseRepository):
    def __init__(self, db: Session):
        super().__init__(db, Account)

    def get_all(self, transactions_limit: int = 10) -> list[Account]:
        ranked_transactions = (
            select(
                Transaction.id,
                Transaction.account_id,
                func.row_number()
                .over(
                    partition_by=Transaction.account_id,
                    order_by=desc(Transaction.date),
                )
                .label("rn"),
            )
        ).subquery()

        recent_transaction_ids = (
            select(ranked_transactions.c.id).where(
                ranked_transactions.c.rn <= transactions_limit
            )
        ).scalar_subquery()

        query = select(Account).options(
            selectinload(
                Account.transactions.and_(Transaction.id.in_(recent_transaction_ids))
            )
        )

        return self.db.scalars(query).unique().all()

    def get_account_by_id(
        self, account_id: int, transactions_limit: int = 10
    ) -> Account | None:
        account: Account = self.db.execute(
            select(Account).where(Account.id == account_id)
        ).scalar_one_or_none()
        account.transactions = self.db.scalars(
            select(Transaction)
            .order_by(desc(Transaction.date))
            .where(Transaction.account_id == account_id)
            .limit(transactions_limit)
        ).all()
        return account
