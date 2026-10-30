# ANCHOR: branch
git switch -c artemis-exercise
# ANCHOR_END: branch
# ANCHOR: expect
git apply docs/tutorial/exercise/1-expect-artemis.patch
# ANCHOR_END: expect
# ANCHOR: red
cargo test --locked --lib demo_catalogue_matches_the_spec_table
# ANCHOR_END: red
# ANCHOR: rename
git apply docs/tutorial/exercise/2-rename-catalogue.patch
# ANCHOR_END: rename
# ANCHOR: green
cargo test --locked --lib demo_catalogue_matches_the_spec_table
# ANCHOR_END: green
# ANCHOR: suite-red
cargo test --locked -- --test-threads=2
# ANCHOR_END: suite-red
# ANCHOR: fixtures
git apply docs/tutorial/exercise/3-follow-fixtures.patch
# ANCHOR_END: fixtures
# ANCHOR: suite-green
cargo test --locked -- --test-threads=2
# ANCHOR_END: suite-green
# ANCHOR: parity
cargo test --locked --test adapter_parity
scripts/quickstart.sh
# ANCHOR_END: parity
# ANCHOR: restore
git restore src/catalogue.rs tests/adapter_http.rs
git switch -
git branch -d artemis-exercise
# ANCHOR_END: restore
