
run:
    cargo run

test:
    cargo test -- --nocapture

build:
    cargo build

bench:
    cargo bench

install:
    cargo install --path .

miri:
    cargo +nightly miri test

generate_data:
    mkdir -p data/
    cat /dev/urandom | base64 2>/dev/null | fold -w 64 | head -n10000 > data/64x10k.txt
