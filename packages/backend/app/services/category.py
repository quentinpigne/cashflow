from sqlalchemy.orm import Session

from app.models.category import Category
from app.schemas.category import CategoryCreate, CategoryUpdate


class CategoryService:
    def __init__(self, db: Session):
        self.db = db

    def get_categories(self, type: str | None) -> list[Category]:
        query = self.db.query(Category).filter(Category.parent_id.is_(None))
        if type:
            query = query.filter(Category.type == type)
        return query.all()

    def create_category(self, category: CategoryCreate) -> Category:
        db_category = Category(**category.model_dump())
        self.db.add(db_category)
        self.db.commit()
        self.db.refresh(db_category)
        return db_category

    def update_category(
        self, category_id: int, category: CategoryUpdate
    ) -> Category | None:
        db_category = self.db.query(Category).filter(Category.id == category_id).first()
        if db_category:
            for key, value in category.model_dump(exclude_unset=True).items():
                setattr(db_category, key, value)
            self.db.commit()
            self.db.refresh(db_category)
        return db_category

    def delete_category(self, category_id: int) -> Category | None:
        db_category = self.db.query(Category).filter(Category.id == category_id).first()
        if db_category:
            self.db.delete(db_category)
            self.db.commit()
        return db_category
