from fastapi import FastAPI

from app.api.v1 import router_v1
from app.core.config import settings

API_PREFIX = "/api"

app = FastAPI(title=settings.PROJECT_NAME, version=settings.PROJECT_VERSION)

app.include_router(router_v1, prefix=API_PREFIX)
