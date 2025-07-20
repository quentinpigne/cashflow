from sqlalchemy.orm import Session

from app.models.bank import Bank
from app.schemas.bank import BankCreate, BankUpdate


def get_banks(db: Session) -> list[Bank]:
    return db.query(Bank).all()


def create_bank(db: Session, bank: BankCreate) -> Bank:
    db_bank = Bank(**bank.model_dump())
    db.add(db_bank)
    db.commit()
    db.refresh(db_bank)
    return db_bank


def update_bank(db: Session, bank_id: int, bank: BankUpdate) -> Bank | None:
    db_bank = db.query(Bank).filter(Bank.id == bank_id).first()
    if db_bank:
        for key, value in bank.model_dump(exclude_unset=True).items():
            setattr(db_bank, key, value)
        db.commit()
        db.refresh(db_bank)
    return db_bank


def delete_bank(db: Session, bank_id: int) -> Bank | None:
    db_bank = db.query(Bank).filter(Bank.id == bank_id).first()
    if db_bank:
        db.delete(db_bank)
        db.commit()
    return db_bank
