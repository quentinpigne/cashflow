from fastapi import FastAPI

from app.api.v1 import user
from app.core.config import settings

app = FastAPI(title=settings.PROJECT_NAME, version=settings.PROJECT_VERSION)

app.include_router(user.router, prefix="/api/v1/user", tags=["user"])
