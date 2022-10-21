FROM rust as build

WORKDIR /app

RUN USER=root cargo new --bin modular-static-archive
COPY Cargo.lock Cargo.toml ./
RUN cargo build --release
RUN rm src/*.rs

COPY src ./src

RUN rm ./target/release/deps/modular-static-archive*
RUN cargo build --release

FROM ubuntu

WORKDIR /app

COPY --from=build /app/target/release/modular-static-archive .

CMD ./modular-static-archive
