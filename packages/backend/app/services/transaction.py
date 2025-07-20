from typing import List, Optional

from sqlalchemy.orm import Session

from app.models.transaction import Transaction
from app.models.account import Account
from app.schemas.transaction import TransactionCreate, TransactionUpdate


def get_transaction(db: Session, transaction_id: int) -> Optional[Transaction]:
    return db.query(Transaction).filter(Transaction.id == transaction_id).first()


def get_transactions(db: Session, skip: int = 0, limit: int = 100) -> List[Transaction]:
    return db.query(Transaction).offset(skip).limit(limit).all()


def get_transactions_by_account_id(
    db: Session, account_id: int, skip: int = 0, limit: int = 100
) -> List[Transaction]:
    return (
        db.query(Transaction)
        .filter(Transaction.account_id == account_id)
        .offset(skip)
        .limit(limit)
        .all()
    )


def create_transaction(db: Session, transaction_in: TransactionCreate) -> Transaction:
    account = db.query(Account).filter(Account.id == transaction_in.account_id).first()
    if not account:
        raise ValueError("Account not found")

    db_transaction = Transaction(**transaction_in.dict())
    db.add(db_transaction)
    db.flush()  # Flush to get the transaction ID and ensure it's in the session

    account.current_balance += db_transaction.amount
    db.add(account)
    db.commit()
    db.refresh(db_transaction)
    db.refresh(account)
    return db_transaction


def update_transaction(
    db: Session, transaction_id: int, transaction_in: TransactionUpdate
) -> Optional[Transaction]:
    db_transaction = db.query(Transaction).filter(Transaction.id == transaction_id).first()
    if not db_transaction:
        return None

    old_amount = db_transaction.amount
    old_account_id = db_transaction.account_id

    for field, value in transaction_in.dict(exclude_unset=True).items():
        setattr(db_transaction, field, value)

    # Handle account balance update if amount or account_id changed
    if db_transaction.account_id != old_account_id:
        # Revert old account balance
        old_account = db.query(Account).filter(Account.id == old_account_id).first()
        if old_account:
            old_account.current_balance -= old_amount
            db.add(old_account)

        # Apply to new account balance
        new_account = (
            db.query(Account).filter(Account.id == db_transaction.account_id).first()
        )
        if new_account:
            new_account.current_balance += db_transaction.amount
            db.add(new_account)
        else:
            raise ValueError("New account not found")
    elif db_transaction.amount != old_amount:
        account = (
            db.query(Account).filter(Account.id == db_transaction.account_id).first()
        )
        if account:
            account.current_balance = (
                account.current_balance - old_amount + db_transaction.amount
            )
            db.add(account)
        else:
            raise ValueError("Account not found for transaction")

    db.commit()
    db.refresh(db_transaction)
    if "account" in locals() and account:
        db.refresh(account)
    if "old_account" in locals() and old_account:
        db.refresh(old_account)
    if "new_account" in locals() and new_account:
        db.refresh(new_account)

    return db_transaction


def delete_transaction(db: Session, transaction_id: int) -> Optional[Transaction]:
    db_transaction = db.query(Transaction).filter(Transaction.id == transaction_id).first()
    if not db_transaction:
        return None

    account = db.query(Account).filter(Account.id == db_transaction.account_id).first()
    if account:
        account.current_balance -= db_transaction.amount
        db.add(account)

    db.delete(db_transaction)
    db.commit()
    if account:
        db.refresh(account)
    return db_transaction
