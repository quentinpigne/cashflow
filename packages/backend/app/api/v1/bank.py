from fastapi import APIRouter, HTTPException

from app.api.deps import BankServiceDep
from app.schemas.bank import Bank, BankCreate, BankUpdate

router = APIRouter()


@router.get("/", response_model=list[Bank])
def read_banks(bank_service: BankServiceDep):
    return bank_service.get_banks()


@router.post("/", response_model=Bank)
def post_bank(bank: BankCreate, bank_service: BankServiceDep):
    return bank_service.create_bank(bank=bank)


@router.put("/{bank_id}", response_model=Bank)
def put_bank(bank_id: int, bank: BankUpdate, bank_service: BankServiceDep):
    db_bank = bank_service.update_bank(bank_id=bank_id, bank=bank)
    if db_bank is None:
        raise HTTPException(status_code=404, detail="Bank not found")
    return db_bank


@router.delete("/{bank_id}", response_model=Bank)
def del_bank(bank_id: int, bank_service: BankServiceDep):
    db_bank = bank_service.delete_bank(bank_id=bank_id)
    if db_bank is None:
        raise HTTPException(status_code=404, detail="Bank not found")
    return db_bank
