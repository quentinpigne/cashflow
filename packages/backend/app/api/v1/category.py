from typing import Optional
from fastapi import APIRouter, HTTPException

from app.api.deps import CategoryServiceDep
from app.schemas.category import Category, CategoryCreate, CategoryUpdate

router = APIRouter()


@router.get("/", response_model=list[Category])
def read_categories(category_service: CategoryServiceDep, type: Optional[str] = None):
    return category_service.get_categories(type=type)


@router.post("/", response_model=Category)
def post_category(category: CategoryCreate, category_service: CategoryServiceDep):
    return category_service.create_category(category=category)


@router.put("/{category_id}", response_model=Category)
def put_category(
    category_id: int,
    category: CategoryUpdate,
    category_service: CategoryServiceDep,
):
    db_category = category_service.update_category(
        category_id=category_id, category=category
    )
    if db_category is None:
        raise HTTPException(status_code=404, detail="Category not found")
    return db_category


@router.delete("/{category_id}", response_model=Category)
def del_category(category_id: int, category_service: CategoryServiceDep):
    db_category = category_service.delete_category(category_id=category_id)
    if db_category is None:
        raise HTTPException(status_code=404, detail="Category not found")
    return db_category
