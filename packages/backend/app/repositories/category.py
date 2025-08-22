from sqlalchemy import select
from sqlalchemy.orm import Session

from app.models.category import Category
from app.repositories.base import BaseRepository


class CategoryRepository(BaseRepository[Category]):
    def __init__(self, db: Session):
        super().__init__(db, Category)

    def get_all(self, type: str | None) -> list[Category]:
        query = select(Category).where(
            Category.parent_id.is_not(None), Category.type == type if type else None
        )
        return self.db.scalars(query).all()
