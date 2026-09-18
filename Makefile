.PHONY: build test lint fmt up down logs multi gc swagger playground-up playground-run playground-down hygiene

# Build everything (workspace + standalone compressor-xdg)
build:
	cargo build --workspace
	cd services/compressor-xdg && cargo build

test:
	cargo test --workspace
	cd services/compressor-xdg && cargo test

lint:
	cargo clippy --workspace --all-targets
	cd services/compressor-xdg && cargo clippy --all-targets

fmt:
	cargo fmt --all
	cd services/compressor-xdg && cargo fmt

# Docker compose stack (testnet)
up:
	docker compose up -d --build

down:
	docker compose down

logs:
	docker compose logs -f --tail 100

# Start the second polyphony instance (parallel backfill / swap-over demo)
multi:
	docker compose --profile multi up -d --build polyphony-b

# Manual one-shot TiKV MVCC garbage collection (the tikv-gc service also runs
# automatically every 10 minutes)
gc:
	docker compose run --rm tikv-gc tikv-gc

# Playground: native services against tiup TiKV (see scripts/playground.sh)
playground-up:
	scripts/playground.sh up

playground-run:
	scripts/playground.sh run

playground-down:
	scripts/playground.sh down

# Regenerate mapi-xdg's OpenAPI documents (run from services/mapi-xdg, writes docs/*/swagger.json)
swagger:
	cd services/mapi-xdg && \
	cargo run --release -p mapi-xdg -- --mode generate-open-api --node-address unused --redis unused

# Check for internal references that must not ship
hygiene:
	@! grep -rniE "gomaestro-api|pkg\.dev|svc\.cluster\.local|maestro-org-development|ssh://|DEPLOY_KEY|haproxy-dataplane" \
		--include="*.rs" --include="*.toml" --include="*.md" --include="*.yml" --include="*.yaml" --include="Dockerfile" --include="*.sh" \
		--exclude-dir=target . \
		| grep -v "^./Makefile" || (echo "hygiene check failed" && exit 1)
	@echo "hygiene check passed"
