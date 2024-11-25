FROM rust:latest AS builder
WORKDIR /usr/src/myapp
COPY . .

RUN apt-get update && apt-get install -y libtesseract-dev clang
RUN cargo update && cargo build -r && cd worker && cargo update && cargo build -r


FROM ubuntu:latest

COPY --from=builder /usr/src/myapp/target/release/hust_ledger_backend /usr/local/bin/hust_ledger_backend
COPY --from=builder /usr/src/myapp/worker/target/release/worker /usr/local/bin/worker

COPY tags.json .

RUN apt-get update && apt-get install -y tesseract-ocr tzdata \
	&& cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
	&& echo "Asia/Shanghai" > /etc/timezone

WORKDIR /env

ENTRYPOINT ["sh", "-c", "worker & hust_ledger_backend"]