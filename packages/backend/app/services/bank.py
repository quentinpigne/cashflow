from app.models.bank import Bank as BankModel
from app.repositories.bank import BankRepository
from app.schemas.bank import Bank, BankCreate, BankUpdate


class BankService:
    def __init__(self, bank_repository: BankRepository):
        self.bank_repository = bank_repository

    def get_banks(self) -> list[Bank]:
        return [Bank.model_validate(bank) for bank in self.bank_repository.get_all()]

    def create_bank(self, bank: BankCreate) -> Bank:
        db_bank: BankModel = BankModel(**bank.model_dump())
        return Bank.model_validate(self.bank_repository.save(db_bank))

    def update_bank(self, bank_id: int, bank: BankUpdate) -> Bank | None:
        db_bank: BankModel | None = self.bank_repository.get_by_id(bank_id)
        if db_bank:
            for key, value in bank.model_dump(exclude_unset=True).items():
                setattr(db_bank, key, value)
                self.bank_repository.save(db_bank)
        return Bank.model_validate(db_bank) if db_bank else None

    def delete_bank(self, bank_id: int) -> Bank | None:
        db_bank: BankModel | None = self.bank_repository.delete_by_id(bank_id)
        return Bank.model_validate(db_bank) if db_bank else None
