CORE_FEATURES := default,standalone,remote
ADVANCED_FEATURES := standalone,proxy,https,record,remote
CARGO_HACK_FEATURE_POWERSET := cargo hack test --feature-powerset --exclude-no-default-features --exclude-all-features
CARGO_HACK_FEATURE_POWERSET_WITH_DEFAULT := $(CARGO_HACK_FEATURE_POWERSET) --features default

.PHONY: setup
setup:
	cargo install cargo-audit
	cargo install --locked cargo-deny
	cargo install cargo-tarpaulin
	cargo install --locked cargo-hack

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
	$(CARGO_HACK_FEATURE_POWERSET) --include-features "$(CORE_FEATURES)" --mutually-exclusive-features default,standalone --mutually-exclusive-features default,remote -- --test-threads=1

.PHONY: core-features-integration-test
core-features-integration-test: clean-coverage
	$(CARGO_HACK_FEATURE_POWERSET) --include-features "$(CORE_FEATURES)" --mutually-exclusive-features default,standalone --mutually-exclusive-features default,remote --test lib -- --test-threads=1

.PHONY: advanced-features-test
advanced-features-test: clean-coverage
	$(CARGO_HACK_FEATURE_POWERSET_WITH_DEFAULT) --include-features "$(ADVANCED_FEATURES)" -- --test-threads=1

.PHONY: advanced-features-integration-test
advanced-features-integration-test: clean-coverage
	$(CARGO_HACK_FEATURE_POWERSET_WITH_DEFAULT) --include-features "$(ADVANCED_FEATURES)" --test lib -- --test-threads=1

.PHONY: advanced-features-test-docker
advanced-features-test-docker: clean-coverage
	docker compose up -d
	HTTPMOCK_TESTS_DISABLE_SIMULATED_STANDALONE_SERVER=1 $(CARGO_HACK_FEATURE_POWERSET_WITH_DEFAULT) --include-features "$(ADVANCED_FEATURES)" -- --test-threads=1

.PHONY: advanced-features-integration-test-docker
advanced-features-integration-test-docker: clean-coverage
	docker compose up -d
	HTTPMOCK_TESTS_DISABLE_SIMULATED_STANDALONE_SERVER=1 $(CARGO_HACK_FEATURE_POWERSET_WITH_DEFAULT) --include-features "$(ADVANCED_FEATURES)" --test lib -- --test-threads=1

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
