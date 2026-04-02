.PHONY: help test unit integration bench bench-verbose bench-serial-parallel bench-throughput bench-scale smoke clean docs check format

FEATURES := --features parsing

help:          # Show available commands
	@echo "Rust Indexer - Make targets"
	@echo "========================="
	@echo "  make test                    Run all tests (unit + integration)"
	@echo "  make unit                    Run only unit tests (library)"
	@echo "  make integration             Run only integration tests"
	@echo "  make smoke                   Run smoke tests only"
	@echo "  make bench                   Run all benchmarks with output"
	@echo "  make bench-serial-parallel   Compare serial vs parallel (200 files)"
	@echo "  make bench-throughput        Throughput at 50, 100, 200 files"
	@echo "  make bench-scale             Large-scale: 100 and 500 files"
	@echo "  make bench-full              All benchmarks with detailed output"
	@echo "  make clean                   Clean build artifacts"
	@echo "  make check                   Check compilation"
	@echo "  make format                  Format code"
	@echo "  make lint                    Run clippy"

test:          # Run all tests (unit + integration)
	cargo test $(FEATURES)

unit:          # Run only unit tests (library module tests)
	cargo test $(FEATURES) --lib

integration:   # Run only integration tests (tests/*.rs)
	cargo test $(FEATURES) --test '*' -- --test-threads=4

smoke:         # Run smoke tests only
	cargo test $(FEATURES) --test 'smoke_*'

bench:         # Run performance benchmarks (with output)
	cargo test $(FEATURES) infra::benchmarks -- --nocapture

bench-serial-parallel:  # Compare serial vs parallel (200 files)
	cargo test $(FEATURES) bench_serial_vs_parallel_comparison -- --nocapture

bench-throughput:  # Throughput at 50, 100, 200 files
	cargo test $(FEATURES) bench_throughput_scales_with_file_count -- --nocapture

bench-scale:   # Large-scale: 100 and 500 files
	cargo test $(FEATURES) bench_index_100_files_all_languages -- --nocapture
	cargo test $(FEATURES) bench_index_500_files_parallel -- --nocapture

bench-full:  # Run all benchmarks with detailed output
	@echo "=== Latency (Rust) ==="
	cargo test $(FEATURES) bench_parse_latency_rust -- --nocapture 2>&1 | grep -E "test|latency"
	@echo ""
	@echo "=== Throughput (Rust/TS/Java) ==="
	cargo test $(FEATURES) bench_throughput -- --nocapture 2>&1 | grep -E "throughput|symbols/s"
	@echo ""
	@echo "=== Concurrency ==="
	cargo test $(FEATURES) bench_concurrent -- --nocapture 2>&1 | grep -E "concurrent|thread_safe"
	@echo ""
	@echo "=== Serial vs Parallel ==="
	cargo test $(FEATURES) bench_serial_vs_parallel_comparison -- --nocapture 2>&1 | grep -E "Serial vs"
	@echo ""
	@echo "=== Scaling ==="
	cargo test $(FEATURES) bench_throughput_scales_with_file_count -- --nocapture 2>&1 | grep -E "files:"
	@echo ""
	@echo "=== 100 files ==="
	cargo test $(FEATURES) bench_index_100_files_all_languages -- --nocapture 2>&1 | grep -E "benchmark:"
	@echo ""
	@echo "=== 500 files ==="
	cargo test $(FEATURES) bench_index_500_files_parallel -- --nocapture 2>&1 | grep -E "benchmark:"

clean:         # Clean build artifacts
	cargo clean

check:         # Check compilation without running
	cargo check $(FEATURES)

format:        # Format code
	cargo fmt

lint:          # Run clippy
	cargo clippy $(FEATURES) -- -D warnings
