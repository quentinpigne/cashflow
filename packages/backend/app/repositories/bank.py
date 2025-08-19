from sqlalchemy.orm import Session

from app.models.bank import Bank
from app.repositories.base import BaseRepository


class BankRepository(BaseRepository[Bank]):
    def __init__(self, db: Session):
        super().__init__(db, Bank)
