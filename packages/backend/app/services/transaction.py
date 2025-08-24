import io
import pandas as pd

from datetime import datetime
from typing import List, Optional

from fastapi import UploadFile

from app.models.account import Account as AccountModel
from app.repositories.account import AccountRepository
from app.schemas.importer import ColumnMapping, ImportConfig
from app.repositories.transaction import TransactionRepository
from app.models.transaction import Transaction as TransactionModel
from app.schemas.transaction import Transaction, TransactionCreate, TransactionUpdate


class TransactionService:
    def __init__(
        self,
        account_repository: AccountRepository,
        transaction_repository: TransactionRepository,
    ):
        self.account_repository = account_repository
        self.transaction_repository = transaction_repository

    def get_transaction(self, transaction_id: int) -> Optional[Transaction]:
        return Transaction.model_validate(
            self.transaction_repository.get_by_id(transaction_id)
        )

    def get_transactions(self, skip: int = 0, limit: int = 100) -> List[Transaction]:
        return [
            Transaction.model_validate(transaction)
            for transaction in self.transaction_repository.get_all(skip, limit)
        ]

    def get_transactions_by_account_id(
        self, account_id: int, skip: int = 0, limit: int = 100
    ) -> List[Transaction]:
        return [
            Transaction.model_validate(transaction)
            for transaction in self.transaction_repository.get_by_account_id(
                account_id,
                skip,
                limit,
            )
        ]

    def create_transaction(self, transaction_in: TransactionCreate) -> Transaction:
        account: AccountModel | None = self.account_repository.get_by_id(
            transaction_in.account_id
        )
        if not account:
            raise ValueError("Account not found")

        db_transaction = TransactionModel(**transaction_in.model_dump())
        self.transaction_repository.save(db_transaction, commit=False)

        account.current_balance += db_transaction.amount
        self.account_repository.save(account)
        return Transaction.model_validate(db_transaction)

    def update_transaction(
        self, transaction_id: int, transaction_in: TransactionUpdate
    ) -> Optional[Transaction]:
        db_transaction: TransactionModel | None = self.transaction_repository.get_by_id(
            transaction_id
        )
        if not db_transaction:
            raise ValueError("Transaction not found")

        old_amount = db_transaction.amount
        old_account_id = db_transaction.account_id

        for field, value in transaction_in.model_dump(exclude_unset=True).items():
            setattr(db_transaction, field, value)

        # Handle account balance update if amount or account_id changed
        if db_transaction.account_id != old_account_id:
            # Revert old account balance
            old_account: AccountModel | None = self.account_repository.get_by_id(
                old_account_id
            )
            if old_account:
                old_account.current_balance -= old_amount
                self.account_repository.save(old_account, commit=False)

            # Apply to new account balance
            new_account: AccountModel | None = self.account_repository.get_by_id(
                db_transaction.account_id
            )
            if new_account:
                new_account.current_balance += db_transaction.amount
                self.account_repository.save(new_account, commit=False)
            else:
                raise ValueError("New account not found")
        elif db_transaction.amount != old_amount:
            account: AccountModel | None = self.account_repository.get_by_id(
                db_transaction.account_id
            )
            if account:
                account.current_balance = (
                    account.current_balance - old_amount + db_transaction.amount
                )
                self.account_repository.save(account, commit=False)
            else:
                raise ValueError("Account not found for transaction")

        self.transaction_repository.save(db_transaction)
        return Transaction.model_validate(db_transaction)

    def delete_transaction(self, transaction_id: int) -> Optional[Transaction]:
        db_transaction: TransactionModel | None = self.transaction_repository.get_by_id(
            transaction_id
        )
        if not db_transaction:
            return None

        account: AccountModel | None = self.account_repository.get_by_id(
            db_transaction.account_id
        )
        if account:
            account.current_balance -= db_transaction.amount
            self.account_repository.save(account, commit=False)

        self.transaction_repository.delete(db_transaction)
        return Transaction.model_validate(db_transaction)

    def import_transactions(
        self, account_id: int, file: UploadFile, config: ImportConfig
    ) -> List[Transaction]:
        account: AccountModel | None = self.account_repository.get_by_id(account_id)
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
            db_transaction = TransactionModel(
                **transaction_in.model_dump(exclude_unset=True)
            )
            self.transaction_repository.save(db_transaction, commit=False)

            account.current_balance = float(account.current_balance) + float(
                db_transaction.amount
            )
            transactions.append(db_transaction)

        self.account_repository.save(account)
        return transactions
