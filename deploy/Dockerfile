# syntax=docker/dockerfile:1
# Image unique de Cashflow (ADR-0015) : le binaire sert l'API et l'interface.

FROM node:24-trixie-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npx ng build

FROM rust:1.98-trixie AS server
WORKDIR /src
# Les requêtes sqlx sont vérifiées à partir du cache .sqlx/ (ADR-0017).
ENV SQLX_OFFLINE=true
COPY Cargo.toml Cargo.lock rustfmt.toml clippy.toml ./
COPY .sqlx .sqlx
COPY crates crates
RUN cargo build --release --locked -p cashflow-server

FROM debian:trixie-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends tzdata ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /data cashflow \
    && mkdir /data && chown cashflow:cashflow /data
COPY --from=server /src/target/release/cashflow /usr/local/bin/cashflow
COPY --from=web /src/web/dist/cashflow/browser /app/web
ENV CASHFLOW_DATA_DIR=/data \
    CASHFLOW_ADDR=0.0.0.0:3000 \
    CASHFLOW_WEB_DIR=/app/web \
    TZ=Europe/Paris
USER cashflow
VOLUME /data
EXPOSE 3000
ENTRYPOINT ["cashflow"]
CMD ["serve"]
