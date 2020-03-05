alias b := build
alias f := fix
alias t := test
alias v := validate

# Builds the project
build:
    cargo build

# Checks the formatting of the project
check_format:
    cargo fmt -- --check

# Generates documentation for public items.
doc:
    cargo doc

# Generates documentation for public and private items.
doc_all:
    cargo doc --document-private-items

# Fixes issues that can be addressed automatically
fix: format

# Formats rust code
format:
    cargo fmt

# Validates code style
lint:
    cargo clippy -- -D absolute_paths_not_starting_with_crate -D anonymous_parameters -D deprecated_in_future -D elided_lifetimes_in_paths -D explicit_outlives_requirements -D indirect_structural_match -D keyword_idents -D macro_use_extern_crate -D meta_variable_misuse -D missing_copy_implementations -D missing_debug_implementations -D missing_docs -D missing_doc_code_examples -D non_ascii_idents -D private_doc_tests -D trivial_casts -D trivial_numeric_casts -D unreachable_pub -D unsafe_code -D unstable_features -D unused_extern_crates -D unused_import_braces -D unused_lifetimes -D unused_results -D warnings -D clippy::cargo -D clippy::nursery -D clippy::pedantic

# Runs tests
test:
    cargo test --verbose --all-features

# Validates the project
validate: check_format build test lint
