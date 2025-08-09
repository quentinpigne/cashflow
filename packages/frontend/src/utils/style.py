import streamlit as st

text_center = "text-align: center;"

flex_centered_container: str = (
    "display: flex; justify-content: space-between; align-items: center;"
)


def common_container_style(color: str) -> list[str]:
    return [
        "padding: 20px;",
        f"background-color: {color};",
        "border-radius: 5px;",
    ]


def get_css(styles: list[str]) -> str:
    return "{\n    " + "\n    ".join(styles) + "\n}"


def local_css(file_name):
    with open(file_name) as f:
        st.markdown(f"<style>{f.read()}</style>", unsafe_allow_html=True)
