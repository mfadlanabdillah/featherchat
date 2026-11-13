# Feather hub in a small container. Only the headless server is built, so the
# image does not carry a desktop window, a browser engine or a database.

FROM rust:1-bookworm AS build
WORKDIR /src

# Copy the workspace manifests first so a rebuild reuses dependency layers.
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY src ./src
COPY web ./web

RUN cargo build --release -p feather-core --bin feather-hub


FROM debian:bookworm-slim

RUN useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin feather \
    && mkdir -p /data \
    && chown feather:feather /data

COPY --from=build /src/target/release/feather-hub /usr/local/bin/feather-hub

ENV FEATHER_BIND=0.0.0.0 \
    FEATHER_PORT=8777 \
    FEATHER_DATA=/data/hub.json

EXPOSE 8777
VOLUME ["/data"]

USER feather
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["feather-hub", "--health"]

ENTRYPOINT ["feather-hub"]
