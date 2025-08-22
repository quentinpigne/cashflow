from app.models.account import Account as AccountModel
from app.repositories.account import AccountRepository
from app.schemas.account import Account, AccountCreate, AccountUpdate


class AccountService:
    def __init__(self, account_repository: AccountRepository):
        self.account_repository = account_repository

    def get_accounts(self) -> list[Account]:
        return [
            Account.model_validate(account)
            for account in self.account_repository.get_all()
        ]

    def get_account_by_id(self, account_id: int) -> Account | None:
        return Account.model_validate(
            self.account_repository.get_account_by_id(account_id)
        )

    def create_account(self, account: AccountCreate) -> Account:
        db_account = AccountModel(**account.model_dump())
        db_account.current_balance = db_account.initial_balance
        return Account.model_validate(self.account_repository.save(db_account))

    def update_account(self, account_id: int, account: AccountUpdate) -> Account | None:
        db_account: AccountModel | None = self.account_repository.get_account_by_id(
            account_id
        )
        if db_account:
            for key, value in account.model_dump(exclude_unset=True).items():
                setattr(db_account, key, value)
            self.account_repository.save(db_account)
        return Account.model_validate(db_account) if db_account else None

    def delete_account(self, account_id: int) -> Account | None:
        db_account: AccountModel | None = self.account_repository.get_account_by_id(
            account_id
        )
        return Account.model_validate(db_account) if db_account else None
