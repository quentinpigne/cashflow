from sqlalchemy.orm import Session

from app.models.account import Account
from app.schemas.account import AccountCreate, AccountUpdate


class AccountService:
    def __init__(self, db: Session):
        self.db = db

    def get_accounts(self) -> list[Account]:
        return self.db.query(Account).all()

    def get_account_by_id(self, account_id: int) -> Account | None:
        return self.db.query(Account).filter(Account.id == account_id).first()

    def create_account(self, account: AccountCreate) -> Account:
        db_account = Account(**account.model_dump())
        db_account.current_balance = db_account.initial_balance
        self.db.add(db_account)
        self.db.commit()
        self.db.refresh(db_account)
        return db_account

    def update_account(self, account_id: int, account: AccountUpdate) -> Account | None:
        db_account = self.db.query(Account).filter(Account.id == account_id).first()
        if db_account:
            for key, value in account.model_dump(exclude_unset=True).items():
                setattr(db_account, key, value)
            self.db.commit()
            self.db.refresh(db_account)
        return db_account

    def delete_account(self, account_id: int) -> Account | None:
        db_account = self.db.query(Account).filter(Account.id == account_id).first()
        if db_account:
            self.db.delete(db_account)
            self.db.commit()
        return db_account
