from typing import Generic, Type, TypeVar

from sqlalchemy import select
from sqlalchemy.orm import Session

T = TypeVar("T")


class BaseRepository(Generic[T]):
    def __init__(self, db: Session, model_class: Type[T]):
        self.db = db
        self.model_class = model_class

    def get_all(self) -> list[T]:
        return self.db.scalars(select(self.model_class)).all()

    def get_by_id(self, entity_id: int) -> T | None:
        return self.db.execute(
            select(self.model_class).where(self.model_class.id == entity_id)
        ).scalar()

    def save(self, entity: T) -> T:
        self.db.add(entity)
        self.db.commit()
        self.db.refresh(entity)
        return entity

    def delete_by_id(self, entity_id: int) -> T | None:
        entity = self.get_by_id(entity_id)
        if entity:
            self.db.delete(entity)
            self.db.commit()
        return entity
