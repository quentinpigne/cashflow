from fastapi import APIRouter, HTTPException, status

from app.api.deps import AccountServiceDep
from app.schemas.account import Account, AccountCreate, AccountUpdate

router = APIRouter()


@router.get("/", response_model=list[Account])
def read_accounts(account_service: AccountServiceDep):
    return account_service.get_accounts()


@router.get("/{account_id}", response_model=Account)
def read_account(account_id: int, account_service: AccountServiceDep):
    db_account = account_service.get_account_by_id(account_id=account_id)
    if db_account is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Account not found"
        )
    return db_account


@router.post("/", response_model=Account)
def post_account(account: AccountCreate, account_service: AccountServiceDep):
    return account_service.create_account(account=account)


@router.put("/{account_id}", response_model=Account)
def put_account(
    account_id: int, account: AccountUpdate, account_service: AccountServiceDep
):
    db_account = account_service.update_account(account_id=account_id, account=account)
    if db_account is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Account not found"
        )
    return db_account


@router.delete("/{account_id}", response_model=Account)
def delete_account_endpoint(account_id: int, account_service: AccountServiceDep):
    db_account = account_service.delete_account(account_id=account_id)
    if db_account is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND, detail="Account not found"
        )
    return db_account
