# ---- builder
FROM rust:1.80 as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# build deps first for caching
RUN mkdir src && echo "fn main(){}" > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
RUN cargo build --release

# ---- runtime (distroless-ish)
FROM gcr.io/distroless/cc-debian12:nonroot
WORKDIR /app
COPY --from=builder /app/target/release/dockmasterbot /usr/local/bin/dockmasterbot
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/dockmasterbot"]