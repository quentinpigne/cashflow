import streamlit as st
from streamlit_extras.floating_button import floating_button
from streamlit_extras.metric_cards import style_metric_cards

from src.dialogs.quick_actions import quick_actions
from src.components.dashboard import setup_main_dashboard, setup_overview
from src.components.sidebar import setup_sidebar
from src.services.api import get_accounts
from src.schemas.account import Account

style_metric_cards(background_color="lightgrey", box_shadow=False)

st.set_page_config(
    page_title="Cashflow",
    layout="wide",
    initial_sidebar_state="expanded",
)

accounts: list[Account] = get_accounts()

setup_sidebar()

setup_overview(accounts)

setup_main_dashboard(accounts)

floating_button("Actions", type="primary", on_click=quick_actions)
