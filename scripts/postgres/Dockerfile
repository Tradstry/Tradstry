# Tradstry Postgres image: pgvector/pgvector:pg18 + ParadeDB pg_search (BM25).
# pgvector → dense vector search; pg_search → real BM25 lexical search, together
# giving true hybrid retrieval (latency.md B3/B4/B5). pg_search requires
# `shared_preload_libraries='pg_search'` at runtime (set in postgres.sh, not here).
#
# PG_ARCH selects the ParadeDB .deb architecture. Default `amd64` keeps the VPS
# build unchanged; local (Apple Silicon) builds pass `--build-arg PG_ARCH=arm64`.
FROM pgvector/pgvector:pg18

ARG PG_SEARCH_VERSION=0.23.5
ARG PG_ARCH=amd64
ADD https://github.com/paradedb/paradedb/releases/download/v${PG_SEARCH_VERSION}/postgresql-18-pg-search_${PG_SEARCH_VERSION}-1PARADEDB-bookworm_${PG_ARCH}.deb /tmp/pg_search.deb

RUN apt-get update \
 && apt-get install -y --no-install-recommends /tmp/pg_search.deb \
 && rm -rf /tmp/pg_search.deb /var/lib/apt/lists/*
