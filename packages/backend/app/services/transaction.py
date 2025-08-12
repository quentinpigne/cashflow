import io
import pandas as pd

from datetime import datetime
from typing import List, Optional

from fastapi import UploadFile
from sqlalchemy.orm import Session

from app.models.transaction import Transaction
from app.models.account import Account
from app.schemas.importer import ColumnMapping, ImportConfig
from app.schemas.transaction import TransactionCreate, TransactionUpdate


class TransactionService:
    def __init__(self, db: Session):
        self.db = db

    def get_transaction(self, transaction_id: int) -> Optional[Transaction]:
        return (
            self.db.query(Transaction).filter(Transaction.id == transaction_id).first()
        )

    def get_transactions(self, skip: int = 0, limit: int = 100) -> List[Transaction]:
        return self.db.query(Transaction).offset(skip).limit(limit).all()

    def get_transactions_by_account_id(
        self, account_id: int, skip: int = 0, limit: int = 100
    ) -> List[Transaction]:
        return (
            self.db.query(Transaction)
            .filter(Transaction.account_id == account_id)
            .offset(skip)
            .limit(limit)
            .all()
        )

    def create_transaction(self, transaction_in: TransactionCreate) -> Transaction:
        account = (
            self.db.query(Account)
            .filter(Account.id == transaction_in.account_id)
            .first()
        )
        if not account:
            raise ValueError("Account not found")

        db_transaction = Transaction(**transaction_in.dict())
        self.db.add(db_transaction)
        self.db.flush()  # Flush to get the transaction ID and ensure it's in the session

        account.current_balance += db_transaction.amount
        self.db.add(account)
        self.db.commit()
        self.db.refresh(db_transaction)
        self.db.refresh(account)
        return db_transaction

    def update_transaction(
        self, transaction_id: int, transaction_in: TransactionUpdate
    ) -> Optional[Transaction]:
        db_transaction = (
            self.db.query(Transaction).filter(Transaction.id == transaction_id).first()
        )
        if not db_transaction:
            return None

        old_amount = db_transaction.amount
        old_account_id = db_transaction.account_id

        for field, value in transaction_in.dict(exclude_unset=True).items():
            setattr(db_transaction, field, value)

        # Handle account balance update if amount or account_id changed
        if db_transaction.account_id != old_account_id:
            # Revert old account balance
            old_account = (
                self.db.query(Account).filter(Account.id == old_account_id).first()
            )
            if old_account:
                old_account.current_balance -= old_amount
                self.db.add(old_account)

            # Apply to new account balance
            new_account = (
                self.db.query(Account)
                .filter(Account.id == db_transaction.account_id)
                .first()
            )
            if new_account:
                new_account.current_balance += db_transaction.amount
                self.db.add(new_account)
            else:
                raise ValueError("New account not found")
        elif db_transaction.amount != old_amount:
            account = (
                self.db.query(Account)
                .filter(Account.id == db_transaction.account_id)
                .first()
            )
            if account:
                account.current_balance = (
                    account.current_balance - old_amount + db_transaction.amount
                )
                self.db.add(account)
            else:
                raise ValueError("Account not found for transaction")

        self.db.commit()
        self.db.refresh(db_transaction)
        if "account" in locals() and account:
            self.db.refresh(account)
        if "old_account" in locals() and old_account:
            self.db.refresh(old_account)
        if "new_account" in locals() and new_account:
            self.db.refresh(new_account)

        return db_transaction

    def delete_transaction(self, transaction_id: int) -> Optional[Transaction]:
        db_transaction = (
            self.db.query(Transaction).filter(Transaction.id == transaction_id).first()
        )
        if not db_transaction:
            return None

        account = (
            self.db.query(Account)
            .filter(Account.id == db_transaction.account_id)
            .first()
        )
        if account:
            account.current_balance -= db_transaction.amount
            self.db.add(account)

        self.db.delete(db_transaction)
        self.db.commit()
        if account:
            self.db.refresh(account)
        return db_transaction

    def import_transactions(
        self, account_id: int, file: UploadFile, config: ImportConfig
    ) -> List[Transaction]:
        account = self.db.query(Account).filter(Account.id == account_id).first()
        if not account:
            raise ValueError("Account not found")

        content = file.file.read()
        file_extension = file.filename.split(".")[-1].lower()

        if file_extension == "xlsx":
            df = pd.read_excel(
                io.BytesIO(content),
                sheet_name=config.sheet_name,
                skiprows=config.rows_to_skip,
            )
        elif file_extension == "csv":
            df = pd.read_csv(
                io.BytesIO(content),
                delimiter=config.delimiter,
                skiprows=config.rows_to_skip,
            )
        else:
            raise ValueError("Unsupported file type")

        transactions = []
        for _, row in df.iterrows():
            transaction_data = {}
            for key, mapping in config.column_mapping.items():
                if isinstance(mapping, ColumnMapping):
                    value = row[mapping.column]
                    if mapping.transformation:
                        transformation = mapping.transformation
                        if transformation["type"] == "add_suffix":
                            value = f"{value}{transformation['suffix']}"
                            transaction_data[key] = value
                else:
                    transaction_data[key] = row[mapping]

            transaction_in = TransactionCreate(
                date=datetime.strptime(transaction_data["date"], "%d/%m/%Y").date(),
                bank_label=transaction_data["bank_label"],
                amount=transaction_data["amount"],
                account_id=account_id,
                category_id=transaction_data.get("category_id"),
            )
            db_transaction = Transaction(**transaction_in.model_dump())
            self.db.add(db_transaction)
            self.db.flush()

            account.current_balance = float(account.current_balance) + float(
                db_transaction.amount
            )
            transactions.append(db_transaction)

        self.db.add(account)
        self.db.commit()
        for transaction in transactions:
            self.db.refresh(transaction)
        self.db.refresh(account)

        return transactions
