FROM rust:latest AS builder
WORKDIR /usr/src/myapp
COPY . .

RUN apt-get update && apt-get install -y libtesseract-dev libleptonica-dev clang
RUN cargo install --path .

FROM ubuntu:latest

COPY --from=builder /usr/local/cargo/bin/hust_ledger_backend /usr/local/bin/hust_ledger_backend

RUN apt-get update && apt-get install -y tesseract-ocr tzdata \
	&& cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
	&& echo "Asia/Shanghai" > /etc/timezone

WORKDIR /env

ENTRYPOINT ["hust_ledger_backend"]