from typing import Annotated, Generator

from fastapi import Depends
from sqlalchemy.orm import Session

from app.core.database import SessionLocal

from app.services.bank import BankService
from app.services.account import AccountService
from app.services.category import CategoryService
from app.services.transaction import TransactionService

from app.repositories.bank import BankRepository


def get_db() -> Generator[Session, None, None]:
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


def get_account_service(db: Annotated[Session, Depends(get_db)]) -> AccountService:
    return AccountService(db)


def get_bank_repository(db: Annotated[Session, Depends(get_db)]) -> BankRepository:
    return BankRepository(db)


def get_bank_service(
    bank_repository: Annotated[BankRepository, Depends(get_bank_repository)],
) -> BankService:
    return BankService(bank_repository)


def get_category_service(db: Annotated[Session, Depends(get_db)]) -> CategoryService:
    return CategoryService(db)


def get_transaction_service(
    db: Annotated[Session, Depends(get_db)],
) -> TransactionService:
    return TransactionService(db)


AccountServiceDep = Annotated[AccountService, Depends(get_account_service)]
BankServiceDep = Annotated[BankService, Depends(get_bank_service)]
CategoryServiceDep = Annotated[CategoryService, Depends(get_category_service)]
TransactionServiceDep = Annotated[TransactionService, Depends(get_transaction_service)]
