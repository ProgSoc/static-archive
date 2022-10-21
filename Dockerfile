FROM rust as build

WORKDIR /app

COPY . .
RUN cargo build --release

FROM ubuntu

WORKDIR /app

COPY --from=build /app/target/release/modular-static-archive .

CMD ./modular-static-archive
