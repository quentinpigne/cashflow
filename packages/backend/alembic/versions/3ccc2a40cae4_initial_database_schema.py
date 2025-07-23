"""initial database schema

Revision ID: 3ccc2a40cae4
Revises:
Create Date: 2025-07-09 15:40:58.917273

"""

from typing import Sequence, Union

from alembic import op
import sqlalchemy as sa


# revision identifiers, used by Alembic.
revision: str = "3ccc2a40cae4"
down_revision: Union[str, Sequence[str], None] = None
branch_labels: Union[str, Sequence[str], None] = None
depends_on: Union[str, Sequence[str], None] = None


def upgrade() -> None:
    bank_table = op.create_table(
        "bank",
        sa.Column("id", sa.Integer, primary_key=True, autoincrement=True),
        sa.Column("name", sa.String(50), nullable=False),
        sa.Column("bank_code", sa.String(5)),
    )
    op.bulk_insert(
        bank_table,
        [
            {"name": "BNP Paribas", "bank_code": "30004"},
            {"name": "Société Générale", "bank_code": "30003"},
            {"name": "Crédit Agricole", "bank_code": "30006"},
            {"name": "Crédit Mutuel", "bank_code": None},
            {"name": "CIC", "bank_code": None},
            {"name": "CIC - Épargne Salariale", "bank_code": None},
            {"name": "La Banque Postale", "bank_code": "20041"},
            {"name": "Crédit du Nord", "bank_code": "30076"},
            {"name": "LCL", "bank_code": "30002"},
            {"name": "HSBC France", "bank_code": None},
            {"name": "Boursorama Banque", "bank_code": "40618"},
            {"name": "Fortuneo", "bank_code": None},
            {"name": "ING", "bank_code": None},
            {"name": "Monabanq", "bank_code": "14690"},
            {"name": "Hello bank!", "bank_code": None},
            {"name": "BforBank", "bank_code": "16218"},
            {"name": "Orange Bank", "bank_code": "18370"},
            {"name": "N26", "bank_code": "20433"},
            {"name": "Revolut", "bank_code": "28233"},
        ],
    )

    account_type_table = op.create_table(
        "account_type",
        sa.Column("id", sa.Integer, primary_key=True, autoincrement=True),
        sa.Column("name", sa.String(50), nullable=False),
        sa.Column("description", sa.Unicode(200)),
    )
    op.bulk_insert(
        account_type_table,
        [
            {"name": "checking", "description": "Compte courant"},
            {"name": "savings", "description": "Compte d'épargne"},
            {"name": "investment", "description": "Compte d'investissement"},
            {"name": "other", "description": "Autre"},
        ],
    )

    account_table = op.create_table(
        "account",
        sa.Column(
            "id",
            sa.Integer,
            primary_key=True,
            autoincrement=True,
        ),
        sa.Column("name", sa.String(50), nullable=False),
        sa.Column("description", sa.Unicode(200)),
        sa.Column("agency_code", sa.String(5), nullable=False),
        sa.Column("account_number", sa.String(11), nullable=False),
        sa.Column("rib_key", sa.String(2), nullable=False),
        sa.Column("iban", sa.String(34), nullable=False),
        sa.Column("bic", sa.String(11), nullable=False),
        sa.Column("currency", sa.String(3), nullable=False, default="EUR"),
        sa.Column("initial_balance", sa.Float, nullable=False, default=0.0),
        sa.Column("current_balance", sa.Float, nullable=False, default=0.0),
        sa.Column(
            "account_type_id",
            sa.Integer,
            sa.ForeignKey("account_type.id"),
            nullable=False,
        ),
        sa.Column("bank_id", sa.Integer, sa.ForeignKey("bank.id"), nullable=False),
    )

    transaction_category_table = op.create_table(
        "transaction_category",
        sa.Column("id", sa.Integer, primary_key=True, autoincrement=True),
        sa.Column("name", sa.String(50), nullable=False),
        sa.Column("type", sa.String(50), nullable=False, default="expense"),
        sa.Column("description", sa.Unicode(200)),
        sa.Column(
            "parent_id",
            sa.Integer,
            sa.ForeignKey("transaction_category.id"),
            nullable=True,
        ),
    )
    op.bulk_insert(
        transaction_category_table,
        [
            {"name": "Logement", "description": ""},
            {"name": "Loyer", "description": "", "parent_id": 1},
            {"name": "Gaz, Électricité, Eau", "description": "", "parent_id": 1},
            {"name": "Décoration", "description": "", "parent_id": 1},
            {"name": "Extérieur et jardin", "description": "", "parent_id": 1},
            {"name": "Logement - Autres", "description": "", "parent_id": 1},
            {"name": "Alimentation & Restauration", "description": ""},
            {"name": "Supermarché / Épicerie", "description": "", "parent_id": 7},
            {"name": "Restaurants", "description": "", "parent_id": 7},
            {"name": "Cafés / Bars", "description": "", "parent_id": 7},
            {"name": "Fast food", "description": "", "parent_id": 7},
            {"name": "Alimentation - Autres", "description": "", "parent_id": 7},
            {"name": "Achats & Shopping", "description": ""},
            {"name": "Vêtements/Chaussures", "description": "", "parent_id": 13},
            {"name": "Achats & Shopping - Autres", "description": "", "parent_id": 13},
            {"name": "Auto & Transports", "description": ""},
            {"name": "Carburant", "description": "", "parent_id": 16},
            {"name": "Péage", "description": "", "parent_id": 16},
            {"name": "Entretien", "description": "", "parent_id": 16},
            {"name": "Auto & Transports - Autres", "description": "", "parent_id": 16},
            {"name": "Abonnements", "description": ""},
            {"name": "Internet", "description": "", "parent_id": 22},
            {"name": "Téléphone", "description": "", "parent_id": 22},
            {"name": "Abonnements - Autres", "description": "", "parent_id": 22},
            {"name": "Salaires", "description": "", "type": "income"},
            {"name": "Allocations et pensions", "description": "", "type": "income"},
            {"name": "Remboursements", "description": "", "type": "income"},
            {"name": "Autres revenus", "description": "", "type": "income"},
        ],
    )

    transaction_table = op.create_table(
        "transaction",
        sa.Column("id", sa.Integer, primary_key=True, autoincrement=True),
        sa.Column("bank_label", sa.String(50), nullable=False),
        sa.Column("custom_label", sa.String(50), nullable=True),
        sa.Column("comment", sa.String(200), nullable=True),
        sa.Column("amount", sa.Float, nullable=False),
        sa.Column("date", sa.Date, nullable=False),
        sa.Column(
            "account_id", sa.Integer, sa.ForeignKey("account.id"), nullable=False
        ),
        sa.Column(
            "category_id",
            sa.Integer,
            sa.ForeignKey("transaction_category.id"),
            nullable=True,
        ),
    )


def downgrade() -> None:
    op.drop_table("category")
    op.drop_table("transaction")
    op.drop_table("account")
    op.drop_table("account_type")
    op.drop_table("bank")
