# build stage
FROM rust:1.83-alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# runtime stage
FROM alpine:3.20 AS runtime
RUN addgroup -S app && adduser -S app -G app
COPY --from=builder /app/target/release/chimera /usr/local/bin/chimera
USER app
ENTRYPOINT ["chimera"]
