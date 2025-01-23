#!/bin/sh

cargo clean
export OPENSSL_LIB_DIR=/usr/local/opt/openssl@1.1/lib/;
export OPENSSL_INCLUDE_DIR=/usr/local/opt/openssl@1.1/include;
export OPENSSL_STATIC=yes

ROARING_ARCH=x86-64-v2

rustup target add x86_64-apple-darwin

cargo build --release --target x86_64-apple-darwin
