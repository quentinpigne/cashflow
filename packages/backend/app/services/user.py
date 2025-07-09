from app.schemas.user import UserOut


def get_user() -> UserOut:
    return UserOut(id=1, username="John Doe", email="john.doe@example.com")
