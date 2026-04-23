pre-commit: check clippy udeps test

check:
    cargo check --all-targets

clippy:
    cargo clippy --all-targets -- -D warnings

udeps:
    cargo +nightly udeps --all-targets

test:
    RUST_LOG="info" cargo test

dev fixture=".tmp/data.sql" db_path="target/tmp":
    mkdir -p {{db_path}}
    rm -f {{db_path}}/db.sqlite
    touch {{db_path}}/db.sqlite 
    sqlx migrate run --database-url "sqlite://{{db_path}}/db.sqlite"
    cat {{fixture}} | sqlite3 {{db_path}}/db.sqlite

    HTTP_PORT=80 COOKIE_KEY=`head -c 32 /dev/urandom | xxd -p -c 0` DATABASE_URL="sqlite://{{db_path}}/db.sqlite" cargo run -p server
