import streamlit as st
from streamlit_extras.stylable_container import stylable_container

from src.schemas.account import Account
from src.components.projection_graph import projection_graph

from src.utils.style import (
    get_css,
    common_container_style,
    flex_centered_container,
    text_center,
)


def setup_overview(accounts: list[Account]) -> None:
    total_assets_col, savings_rate_col, emergency_months_col, financial_score_col = (
        st.columns(4)
    )

    with total_assets_col:
        st.metric(
            "**Patrimoine total**",
            f"{round(sum(account['current_balance'] for account in accounts), 2)} €",
            delta="1000 (+10%)",
        )

    with savings_rate_col:
        st.metric(
            "**Taux d'épargne**",
            "73%",
            delta="2,3%",
        )

    with emergency_months_col:
        st.metric(
            "**Mois d'urgence**",
            2.3,
            delta="stable",
        )

    with financial_score_col:
        st.metric(
            "**Score financier**",
            "A+",
            delta="Amélioré",
        )


def setup_main_dashboard(accounts: list[Account]) -> None:
    main_left_col, main_center_col, main_right_col = st.columns([1, 2, 1])

    with main_left_col:
        with st.container(border=True):
            st.markdown("#### 📊 Mes Comptes")

            for index, account in enumerate(accounts):
                account_card(account, index)

    with main_center_col:
        with st.container(border=True):
            st.markdown("#### 💳 Budget & Dépenses")
            budget_status_col, budget_details_col = st.columns(2)
            with budget_status_col:
                budget_status_card(78)
            with budget_details_col:
                with stylable_container(
                    key="expenses_container",
                    css_styles=get_css(common_container_style("lightgrey")),
                ):
                    budge_details_card("🍕 Alimentation", 750)
                    budge_details_card("🚗 Transport", 750)
                    budge_details_card("🎭 Loisirs", 750)
                    budge_details_card("🏠 Logement", 750)

        with st.container(border=True):
            st.markdown("#### 📈 Projection")
            projection_graph()

        with st.container(border=True):
            with stylable_container(
                key="investments_container",
                css_styles=get_css(common_container_style("lightgrey")),
            ):
                st.html(
                    f'<div style="{flex_centered_container}"><h4>📊 Performance Investissements</h4><strong style="color: green;">+12.4%</strong></div>'
                )
                actions_col, bonds_col = st.columns(2)
                cryptos_col, cash_col = st.columns(2)
                with actions_col:
                    investment_container(
                        key="actions_container",
                        color="red",
                        name="Actions",
                        value=15,
                    )
                with bonds_col:
                    investment_container(
                        key="bonds_container",
                        color="lightblue",
                        name="Obligations",
                        value=15,
                    )
                with cryptos_col:
                    investment_container(
                        key="crypto_container",
                        color="orange",
                        name="Cryptos",
                        value=15,
                    )
                with cash_col:
                    investment_container(
                        key="cash_container",
                        color="green",
                        name="Cash",
                        value=15,
                    )

    with main_right_col:
        with st.container(border=True):
            st.markdown("#### 🎯 KPIs Clés")
            with stylable_container(
                key="monthly_savings_container",
                css_styles=get_css([*common_container_style("lightgrey"), text_center]),
            ):
                st.html("<strong style='font-size: 1.5em;'>1523 €</strong>")
                st.html("<strong style='font-size: 1em;'>Épargne mensuelle</strong>")
            with stylable_container(
                key="burn_rate_container",
                css_styles=get_css([*common_container_style("lightgrey"), text_center]),
            ):
                st.html("<strong style='font-size: 1.5em;'>2180 €</strong>")
                st.html("<strong style='font-size: 1em;'>Burn rate</strong>")
            with stylable_container(
                key="objectif_container",
                css_styles=get_css([*common_container_style("lightgrey"), text_center]),
            ):
                st.html("<strong style='font-size: 1.5em;'>92%</strong>")
                st.html("<strong style='font-size: 1em;'>Objectif atteint</strong>")


def account_card(account: Account, index: int) -> None:
    with stylable_container(
        key="account-container",
        css_styles=get_css(common_container_style("lightgrey")),
    ):
        st.html(f"<strong style='font-size: 1.25rem'>{account['name']}</strong>")
        st.badge(account["account_type"]["name"])
        st.html(
            f"<strong style='font-size: 1.5rem'>{round(account['current_balance'], 2)} €</strong>"
        )
        st.markdown(f"**{1000}€ ce mois**")
        if st.button("Details", key=index, type="primary"):
            st.session_state.update({"account_id": account["id"]})


def budget_status_card(budget: float) -> None:
    with stylable_container(
        key="budget_status_container",
        css_styles=get_css([*common_container_style("lightgrey"), text_center]),
    ):
        st.html(
            f'<strong style="font-size: 20px; color: {"red" if budget > 70 else "green"}">{budget}%</strong>'
        )
        st.html('<p style="text-align: center; color: grey">Budget mensuel utilisé</p>')
        st.progress(budget)


def budge_details_card(label: str, amount: float) -> None:
    st.html(
        f'<div style="{flex_centered_container}"><p>{label}</p><p>{round(amount, 2)}€</p></div>'
    )


def investment_container(key: str, color: str, name: str, value: float) -> None:
    with stylable_container(
        key,
        css_styles=get_css(
            [
                *common_container_style("white"),
                text_center,
                "border: 1px solid black;",
            ]
        ),
    ):
        st.html(
            f"<div style='{flex_centered_container}'><div style='display: flex; align-items: baseline; gap: 10px;'><div style='width: 10px; height: 10px; border-radius: 50%; background-color: {color};'></div><p>{name}</p></div><p>{round(value, 2)}%</div>"
        )
