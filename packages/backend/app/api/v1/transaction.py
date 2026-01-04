import json
from datetime import date
from typing import Annotated, List, Optional

from fastapi import APIRouter, HTTPException, UploadFile, File, Form

from app.api.deps import TransactionServiceDep
from app.schemas.transaction import Transaction, TransactionCreate, TransactionUpdate
from app.schemas.importer import ImportConfig

router = APIRouter()


@router.get("/", response_model=List[Transaction])
def read_transactions(
    transaction_service: TransactionServiceDep,
    account_id: Optional[int] = None,
    start_date: Optional[date] = None,
    end_date: Optional[date] = None,
    skip: int = 0,
    limit: int = 100,
):
    if account_id:
        return transaction_service.get_transactions_by_account_id(
            account_id=account_id,
            start_date=start_date,
            end_date=end_date,
            skip=skip,
            limit=limit,
        )
    return transaction_service.get_transactions(
        start_date=start_date, end_date=end_date, skip=skip, limit=limit
    )


@router.get("/{transaction_id}", response_model=Transaction)
def read_transaction(
    transaction_service: TransactionServiceDep,
    transaction_id: int,
):
    transaction = transaction_service.get_transaction(transaction_id)
    if not transaction:
        raise HTTPException(status_code=404, detail="Transaction not found")
    return transaction


@router.post("/", response_model=Transaction)
def post_transaction(
    transaction_service: TransactionServiceDep,
    transaction_in: TransactionCreate,
):
    try:
        return transaction_service.create_transaction(transaction_in)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@router.put("/{transaction_id}", response_model=Transaction)
def put_transaction(
    transaction_service: TransactionServiceDep,
    transaction_id: int,
    transaction_in: TransactionUpdate,
):
    try:
        transaction = transaction_service.update_transaction(
            transaction_id, transaction_in
        )
        if not transaction:
            raise HTTPException(status_code=404, detail="Transaction not found")
        return transaction
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))


@router.delete("/{transaction_id}", response_model=Transaction)
def del_transaction(
    transaction_service: TransactionServiceDep,
    transaction_id: int,
):
    transaction = transaction_service.delete_transaction(transaction_id)
    if not transaction:
        raise HTTPException(status_code=404, detail="Transaction not found")
    return transaction


@router.post("/import", response_model=List[Transaction])
def import_data(
    transaction_service: TransactionServiceDep,
    account_id: int,
    config: Annotated[str, Form()],
    file: Annotated[UploadFile, File()],
):
    try:
        config_dict = json.loads(config)
        import_config = ImportConfig(**config_dict)

        return transaction_service.import_transactions(
            account_id,
            file,
            import_config,
        )
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Error importing file: {str(e)}")
