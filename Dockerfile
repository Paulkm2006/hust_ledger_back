FROM rust:latest AS builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

FROM ubuntu:latest

COPY --from=builder /usr/local/cargo/bin/hust_ledger_backend /usr/local/bin/hust_ledger_backend

ENTRYPOINT ["hust_ledger_backend"]