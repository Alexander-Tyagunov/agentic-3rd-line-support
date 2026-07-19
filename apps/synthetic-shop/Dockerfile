# Multi-stage: build a release binary, run on a distroless image.
FROM rust:1-slim AS build
WORKDIR /app
COPY Cargo.toml ./
# Cache deps
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Distroless runtime (no shell, non-root, minimal surface).
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /app/target/release/synthetic-shop /usr/local/bin/synthetic-shop
ENV PORT=8080
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/synthetic-shop"]
