from sqlalchemy.orm import Session

from app.models.category import Category
from app.schemas.category import CategoryCreate, CategoryUpdate


def get_categories(type: str | None, db: Session) -> list[Category]:
    query = db.query(Category).filter(Category.parent_id.is_(None))
    if type:
        query = query.filter(Category.type == type)
    return query.all()


def create_category(db: Session, category: CategoryCreate) -> Category:
    db_category = Category(**category.model_dump())
    db.add(db_category)
    db.commit()
    db.refresh(db_category)
    return db_category


def update_category(
    db: Session, category_id: int, category: CategoryUpdate
) -> Category | None:
    db_category = db.query(Category).filter(Category.id == category_id).first()
    if db_category:
        for key, value in category.model_dump(exclude_unset=True).items():
            setattr(db_category, key, value)
        db.commit()
        db.refresh(db_category)
    return db_category


def delete_category(db: Session, category_id: int) -> Category | None:
    db_category = db.query(Category).filter(Category.id == category_id).first()
    if db_category:
        db.delete(db_category)
        db.commit()
    return db_category
