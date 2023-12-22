
run:
    cargo run

test:
    cargo test -- --nocapture

build:
    cargo build

install:
    cargo install --path .

miri:
    cargo +nightly miri test
