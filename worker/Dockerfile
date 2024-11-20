FROM rust:latest AS builder
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .

FROM ubuntu:latest

COPY --from=builder /usr/local/cargo/bin/worker /usr/local/bin/hust_ledger_backend_worker

RUN cd

COPY config.toml config.toml

ENTRYPOINT ["hust_ledger_backend_worker"]