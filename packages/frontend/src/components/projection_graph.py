import pandas as pd
import streamlit as st


def projection_graph() -> None:
    # Create mock graph with years from 2023 to 2030 in x and fake total assets values in y (conservative and optimistic)
    st.line_chart(
        pd.DataFrame(
            {
                "conservative": [1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400],
                "optimistic": [1000, 1300, 1500, 1700, 1900, 2100, 2300, 2500],
                "realistic": [1000, 1300, 1400, 1500, 1600, 1700, 1800, 1900],
                "pessimistic": [1000, 1100, 1300, 1400, 1500, 1600, 1700, 1800],
            },
            index=pd.date_range("2023", "2031", freq="YE").year,
        )
    )
