# Console image: builds the Leptos WASM UI with trunk, builds the axum backend,
# and serves the bundle on a distroless runtime. Build context is ticket-system/:
#   gcloud builds submit ticket-system --tag IMAGE --timeout=1800

# ---- Stage 1: build the WASM UI (trunk) ----
FROM rust:1-bookworm AS ui
RUN apt-get update && apt-get install -y --no-install-recommends wget ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && rustup target add wasm32-unknown-unknown \
  && wget -qO /tmp/trunk.tar.gz \
     https://github.com/trunk-rs/trunk/releases/latest/download/trunk-x86_64-unknown-linux-gnu.tar.gz \
  && tar -xzf /tmp/trunk.tar.gz -C /usr/local/bin trunk && chmod +x /usr/local/bin/trunk
WORKDIR /src
COPY . .
RUN cd ui && trunk build --release   # emits ui/dist

# ---- Stage 2: build the backend ----
FROM rust:1-bookworm AS backend
WORKDIR /src
COPY . .
RUN cargo build --release -p ticket-backend

# ---- Stage 3: distroless runtime ----
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=backend /src/target/release/ticket-backend /usr/local/bin/ticket-backend
COPY --from=ui /src/ui/dist ./dist
ENV UI_DIST=/app/dist
ENV PORT=8080
ENTRYPOINT ["/usr/local/bin/ticket-backend"]
