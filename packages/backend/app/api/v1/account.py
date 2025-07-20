from fastapi import APIRouter, Depends, HTTPException, status
from sqlalchemy.orm import Session

from app.api import deps
from app.schemas.account import Account, AccountCreate, AccountUpdate
from app.services.account import (
    create_account,
    get_accounts,
    get_account_by_id,
    update_account,
    delete_account,
)

router = APIRouter()


@router.get("/", response_model=list[Account])
def read_accounts(db: Session = Depends(deps.get_db)):
    return get_accounts(db=db)


@router.get("/{account_id}", response_model=Account)
def read_account(account_id: int, db: Session = Depends(deps.get_db)):
    db_account = get_account_by_id(db=db, account_id=account_id)
    if db_account is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Account not found"
        )
    return db_account


@router.post("/", response_model=Account)
def post_account(account: AccountCreate, db: Session = Depends(deps.get_db)):
    return create_account(db=db, account=account)


@router.put("/{account_id}", response_model=Account)
def put_account(
    account_id: int, account: AccountUpdate, db: Session = Depends(deps.get_db)
):
    db_account = update_account(db=db, account_id=account_id, account=account)
    if db_account is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Account not found"
        )
    return db_account


@router.delete("/{account_id}", response_model=Account)
def delete_account_endpoint(account_id: int, db: Session = Depends(deps.get_db)):
    db_account = delete_account(db=db, account_id=account_id)
    if db_account is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Account not found"
        )
    return db_account
