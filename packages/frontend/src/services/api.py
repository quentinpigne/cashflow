import requests

import streamlit as st

from src.schemas.account import Account

BASE_URL = "http://localhost:8000/api/v1"


@st.cache_data
def get_accounts() -> list[Account]:
    response = requests.get(f"{BASE_URL}/accounts")
    return response.json()


@st.cache_data
def get_account(account_id: int) -> Account:
    response = requests.get(f"{BASE_URL}/accounts/{account_id}")
    return response.json()
