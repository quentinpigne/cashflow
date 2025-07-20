from typing import Optional
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session

from app.api import deps
from app.schemas.category import Category, CategoryCreate, CategoryUpdate
from app.services.category import (
    create_category,
    delete_category,
    get_categories,
    update_category,
)

router = APIRouter()


@router.get("/", response_model=list[Category])
def read_categories(type: Optional[str] = None, db: Session = Depends(deps.get_db)):
    return get_categories(type=type, db=db)


@router.post("/", response_model=Category)
def post_category(category: CategoryCreate, db: Session = Depends(deps.get_db)):
    return create_category(db=db, category=category)


@router.put("/{category_id}", response_model=Category)
def put_category(
    category_id: int, category: CategoryUpdate, db: Session = Depends(deps.get_db)
):
    db_category = update_category(db=db, category_id=category_id, category=category)
    if db_category is None:
        raise HTTPException(status_code=404, detail="Category not found")
    return db_category


@router.delete("/{category_id}", response_model=Category)
def del_category(category_id: int, db: Session = Depends(deps.get_db)):
    db_category = delete_category(db=db, category_id=category_id)
    if db_category is None:
        raise HTTPException(status_code=404, detail="Category not found")
    return db_category
