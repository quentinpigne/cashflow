from app.repositories.category import CategoryRepository
from app.models.category import Category as CategoryModel
from app.schemas.category import Category, CategoryCreate, CategoryUpdate


class CategoryService:
    def __init__(self, category_repository: CategoryRepository):
        self.category_repository = category_repository

    def get_categories(self, type: str | None) -> list[Category]:
        return [
            Category.model_validate(category)
            for category in self.category_repository.get_all(type)
        ]

    def create_category(self, category: CategoryCreate) -> Category:
        db_category = CategoryModel(**category.model_dump())
        return Category.model_validate(self.category_repository.save(db_category))

    def update_category(
        self, category_id: int, category: CategoryUpdate
    ) -> Category | None:
        db_category: CategoryModel | None = self.category_repository.get_by_id(
            category_id
        )
        if db_category:
            for key, value in category.model_dump(exclude_unset=True).items():
                setattr(db_category, key, value)
                self.category_repository.save(db_category)
        return Category.model_validate(db_category) if db_category else None

    def delete_category(self, category_id: int) -> Category | None:
        db_category: CategoryModel | None = self.category_repository.delete_by_id(
            category_id
        )
        return Category.model_validate(db_category) if db_category else None
