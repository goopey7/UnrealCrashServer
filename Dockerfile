# Build
FROM rust:alpine3.22 AS builder
WORKDIR /app
RUN apk add --no-cache build-base musl-dev perl pkgconfig openssl-dev
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY templates ./templates
RUN cargo build --release

# Runtime
FROM alpine:latest
WORKDIR /app
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/UECrashServer .
EXPOSE 8080
EXPOSE 8081
RUN mkdir /crashes
CMD ["./UECrashServer", "-p", "/crashes"]
