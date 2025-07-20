from fastapi import APIRouter

from app.api.v1 import account, category, bank, transaction

router_v1 = APIRouter(prefix="/v1")

router_v1.include_router(account.router, prefix="/accounts", tags=["account"])
router_v1.include_router(category.router, prefix="/categories", tags=["category"])
router_v1.include_router(bank.router, prefix="/banks", tags=["bank"])
router_v1.include_router(transaction.router, prefix="/transactions", tags=["transaction"])
