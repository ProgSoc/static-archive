FROM rust as build


RUN USER=root cargo new --bin app
WORKDIR /app

COPY Cargo.lock Cargo.toml ./
RUN cargo build --release
RUN rm src/*.rs

COPY src ./src
COPY html ./html

RUN rm ./target/release/deps/static_archive*
RUN cargo build --release

FROM ubuntu

WORKDIR /app

COPY --from=build /app/target/release/static-archive .

CMD ./static-archive
