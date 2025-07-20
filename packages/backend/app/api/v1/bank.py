from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session

from app.api import deps
from app.schemas.bank import Bank, BankCreate, BankUpdate
from app.services.bank import (
    create_bank,
    delete_bank,
    get_banks,
    update_bank,
)

router = APIRouter()


@router.get("/", response_model=list[Bank])
def read_banks(db: Session = Depends(deps.get_db)):
    return get_banks(db=db)


@router.post("/", response_model=Bank)
def post_bank(bank: BankCreate, db: Session = Depends(deps.get_db)):
    return create_bank(db=db, bank=bank)


@router.put("/{bank_id}", response_model=Bank)
def put_bank(bank_id: int, bank: BankUpdate, db: Session = Depends(deps.get_db)):
    db_bank = update_bank(db=db, bank_id=bank_id, bank=bank)
    if db_bank is None:
        raise HTTPException(status_code=404, detail="Bank not found")
    return db_bank


@router.delete("/{bank_id}", response_model=Bank)
def del_bank(bank_id: int, db: Session = Depends(deps.get_db)):
    db_bank = delete_bank(db=db, bank_id=bank_id)
    if db_bank is None:
        raise HTTPException(status_code=404, detail="Bank not found")
    return db_bank
