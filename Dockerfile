FROM rust as build

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update
RUN apt install musl-tools -y

RUN USER=root cargo new --bin app
WORKDIR /app

COPY Cargo.lock Cargo.toml ./

RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm src/*.rs

COPY . .

RUN rm ./target/x86_64-unknown-linux-musl/release/deps/static_archive*
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine

WORKDIR /app

COPY --from=build /app/target/x86_64-unknown-linux-musl/release/static-archive .

CMD ./static-archive
