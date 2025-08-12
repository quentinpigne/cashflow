from sqlalchemy.orm import Session

from app.models.bank import Bank
from app.schemas.bank import BankCreate, BankUpdate


class BankService:
    def __init__(self, db: Session):
        self.db = db

    def get_banks(self) -> list[Bank]:
        return self.db.query(Bank).all()

    def create_bank(self, bank: BankCreate) -> Bank:
        db_bank = Bank(**bank.model_dump())
        self.db.add(db_bank)
        self.db.commit()
        self.db.refresh(db_bank)
        return db_bank

    def update_bank(self, bank_id: int, bank: BankUpdate) -> Bank | None:
        db_bank = self.db.query(Bank).filter(Bank.id == bank_id).first()
        if db_bank:
            for key, value in bank.model_dump(exclude_unset=True).items():
                setattr(db_bank, key, value)
            self.db.commit()
            self.db.refresh(db_bank)
        return db_bank

    def delete_bank(self, bank_id: int) -> Bank | None:
        db_bank = self.db.query(Bank).filter(Bank.id == bank_id).first()
        if db_bank:
            self.db.delete(db_bank)
            self.db.commit()
        return db_bank
