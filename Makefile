.PHONY: setup
setup:
	cargo install cargo-audit
	cargo install --locked cargo-deny
	cargo install cargo-tarpaulin
	cargo install cargo-hack

.PHONY: test-full
test-full:
	docker compose up -d
	HTTPMOCK_TESTS_DISABLE_SIMULATED_STANDALONE_SERVER=1 cargo hack test --feature-powerset --exclude-features https

.PHONY: check
check:
	cargo fmt --check
	cargo clippy
	cargo audit
	cargo deny check

.PHONY: coverage
coverage:
	cargo tarpaulin --out

.PHONY: coverage-full
coverage-full: clean-coverage
	cargo tarpaulin --config tarpaulin.full.toml --out

.PHONY: core-features-test
core-features-test: clean-coverage
	./scripts/test_all_feature_sets.sh "standalone,remote,remote-https,http2,cookies"

.PHONY: core-features-integration-test
core-features-integration-test: clean-coverage
	./scripts/test_all_feature_sets.sh "standalone,remote,remote-https,http2,cookies" "tests"

.PHONY: advanced-features-test
advanced-features-test: clean-coverage
	./scripts/test_all_feature_sets.sh "https,proxy,record,standalone,remote-https,remote"

.PHONY: advanced-features-integration-test
advanced-features-integration-test: clean-coverage
	./scripts/test_all_feature_sets.sh "https,proxy,record,standalone,remote-https,remote" "tests"

.PHONY: coverage-debug
coverage-debug:
	 RUST_BACKTRACE=1 RUST_LOG=trace cargo tarpaulin --out -- --nocapture

.PHONY: clean-coverage
clean-coverage:
	rm -f *.profraw
	rm -f cobertura.xml
	rm -f tarpaulin-report.html

.PHONY: clean-coverage
clean: clean-coverage
	cargo clean

.PHONY: certs
certs:
	rm -rf certs
	mkdir certs
	cd certs && openssl genrsa -out ca.key 2048
	cd certs && openssl req -x509 -new -nodes -key ca.key -sha256 -days 36525 -out ca.pem -subj "/CN=httpmock"

.PHONY: docker
docker:
	docker-compose build --no-cache
	docker-compose up

.PHONY: docs
docs:
	rm -rf tools/target/generated && mkdir -p tools/target/generated
	cd tools && cargo run --bin extract_docs
	cd tools && cargo run --bin extract_code
	cd tools && cargo run --bin extract_groups
	cd tools && cargo run --bin extract_example_tests
	rm -rf docs/website/generated && cp -r tools/target/generated docs/website/generated
	cd docs/website && npm install && npm run generate-docs

.PHONY: fmt
fmt:
	cargo fmt
	cargo fix --allow-dirty

.PHONY: test-wasm
test-wasm:
	cd wasm-test && wasm-pack test --node
	cd wasm-test && wasm-pack test --headless --chrome

.PHONY: test-wasi
test-wasi:
ifeq ($(PLATFORM),mac)
	$(eval CC := /opt/homebrew/opt/llvm/bin/clang)
endif
	command -v wasmtime >/dev/null 2>&1 || cargo install --locked wasmtime-cli
	rustup +nightly-2024-10-15 target add wasm32-wasip2
	cd wasi-test && env PATH=$(HOME)/.cargo/bin:$$PATH CC=$(CC) cargo +nightly-2024-10-15 test --target wasm32-wasip2 --tests -v
