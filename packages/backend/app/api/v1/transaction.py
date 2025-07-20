from typing import List, Optional

from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session

from app.api.deps import get_db
from app.schemas.transaction import Transaction, TransactionCreate, TransactionUpdate
from app.services.transaction import (
    get_transactions_by_account_id,
    get_transactions,
    get_transaction,
    create_transaction,
    update_transaction,
    delete_transaction,
)

router = APIRouter()


@router.get("/", response_model=List[Transaction])
def read_transactions(
    account_id: Optional[int] = None,
    skip: int = 0,
    limit: int = 100,
    db: Session = Depends(get_db),
):
    if account_id:
        return get_transactions_by_account_id(
            db, account_id=account_id, skip=skip, limit=limit
        )
    return get_transactions(db, skip=skip, limit=limit)


@router.get("/{transaction_id}", response_model=Transaction)
def read_transaction(
    transaction_id: int,
    db: Session = Depends(get_db),
):
    transaction = get_transaction(db, transaction_id)
    if not transaction:
        raise HTTPException(status_code=404, detail="Transaction not found")
    return transaction


@router.post("/", response_model=Transaction)
def post_transaction(
    transaction_in: TransactionCreate,
    db: Session = Depends(get_db),
):
    try:
        return create_transaction(db, transaction_in)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@router.put("/{transaction_id}", response_model=Transaction)
def put_transaction(
    transaction_id: int,
    transaction_in: TransactionUpdate,
    db: Session = Depends(get_db),
):
    try:
        transaction = update_transaction(db, transaction_id, transaction_in)
        if not transaction:
            raise HTTPException(status_code=404, detail="Transaction not found")
        return transaction
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@router.delete("/{transaction_id}", response_model=Transaction)
def del_transaction(
    transaction_id: int,
    db: Session = Depends(get_db),
):
    transaction = delete_transaction(db, transaction_id)
    if not transaction:
        raise HTTPException(status_code=404, detail="Transaction not found")
    return transaction
