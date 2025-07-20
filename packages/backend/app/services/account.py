from sqlalchemy.orm import Session

from app.models.account import Account
from app.schemas.account import AccountCreate, AccountUpdate


def get_accounts(db: Session) -> list[Account]:
    return db.query(Account).all()


def get_account_by_id(db: Session, account_id: int) -> Account | None:
    return db.query(Account).filter(Account.id == account_id).first()


def create_account(db: Session, account: AccountCreate) -> Account:
    db_account = Account(**account.model_dump())
    db_account.current_balance = db_account.initial_balance
    db.add(db_account)
    db.commit()
    db.refresh(db_account)
    return db_account


def update_account(
    db: Session, account_id: int, account: AccountUpdate
) -> Account | None:
    db_account = db.query(Account).filter(Account.id == account_id).first()
    if db_account:
        for key, value in account.model_dump(exclude_unset=True).items():
            setattr(db_account, key, value)
        db.commit()
        db.refresh(db_account)
    return db_account


def delete_account(db: Session, account_id: int) -> Account | None:
    db_account = db.query(Account).filter(Account.id == account_id).first()
    if db_account:
        db.delete(db_account)
        db.commit()
    return db_account
