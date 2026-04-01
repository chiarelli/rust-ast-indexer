# Project CI & Usage

This repository contains Rust indexer and related tooling.

## CI
The GitHub Actions workflow `.github/workflows/ci.yml` runs format checks, clippy, unit tests and the smoke test that exercises the incremental indexing using Git.

## Running smoke tests locally

cd rust_indexer && cargo test --test smoke_incremental_git -- --nocapture
