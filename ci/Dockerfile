FROM rust:1.94.0-alpine3.23

RUN apk update && apk add --no-cache nodejs alpine-sdk openssl-dev openssl-libs-static curl fontconfig-dev freetype-dev zlib-static clang18-libclang

RUN rustup component add clippy

RUN rustup toolchain install nightly

RUN cargo +nightly install cargo-udeps --locked

RUN curl -fsSLo /usr/local/bin/tailwindcss \
  https://github.com/tailwindlabs/tailwindcss/releases/download/v4.1.7/tailwindcss-linux-x64-musl \
  && chmod +x /usr/local/bin/tailwindcss
