# ANCHOR: serve
cargo build --locked
SENU_BIND=127.0.0.1:3000 SENU_WEB_USER=demo SENU_WEB_PASSWORD=demo-password \
  SENU_API_TOKEN=demo-api-token ./target/debug/senu serve
# ANCHOR_END: serve
