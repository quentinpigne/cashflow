import streamlit as st


def setup_sidebar() -> None:
    with st.sidebar:
        st.title("💰 Cashflow")

        if st.button("📊 Dashboard", type="tertiary"):
            st.switch_page("streamlit_app.py")

        if st.button("🏦 Comptes", type="tertiary"):
            (st.switch_page("streamlit_app.py"),)

        if st.button("💳 Transactions", type="tertiary"):
            st.switch_page("streamlit_app.py")

        if st.button("📋 Budget", type="tertiary"):
            st.switch_page("streamlit_app.py")

        st.header("Analytics")

        if st.button("📈 Rapports", type="tertiary"):
            st.switch_page("streamlit_app.py")

        if st.button("🔮 Projections", type="tertiary"):
            st.switch_page("streamlit_app.py")

        if st.button("📊 Investissements", type="tertiary"):
            st.switch_page("streamlit_app.py")

        st.header("Paramètres")

        if st.button("🎯 Objectifs", type="tertiary"):
            st.switch_page("streamlit_app.py")

        if st.button("⚙️ Paramètres", type="tertiary"):
            st.switch_page("streamlit_app.py")

        if st.button("📤 Export", type="tertiary"):
            st.switch_page("streamlit_app.py")
