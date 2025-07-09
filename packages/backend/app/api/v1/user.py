from fastapi import APIRouter

from app.schemas.user import UserOut
from app.services.user import get_user

router = APIRouter()


@router.get("/", response_model=UserOut)
def user() -> UserOut:
    return get_user()
