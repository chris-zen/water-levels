.DEFAULT_GOAL := local-all

SHELL := /bin/bash

RUST_VERSION ?= $(shell head -n 1 rust-toolchain)
TEST_RUST_LOG ?= "debug"
DOCKER_REGISTRY ?= docker.io
WS_URL ?= ws://127.0.0.1:9002

KERNEL := $(shell uname -s)
ifeq ($(KERNEL),Linux)
	OS := linux
endif
ifeq ($(KERNEL),Darwin)
	OS := macos
endif

BUILD_PROFILE := release
BUILD_PROFILE_ARG := --release
TEST_PROFILE := test
TEST_PROFILE_ARG := 

PROJECT_DIR := $(realpath $(CURDIR))
HASH_VERSION := $(shell date -u +%Y%m%d%H%M)-$(shell (git rev-parse HEAD 2>/dev/null | cut -c1-8 | tr '[:upper:]' '[:lower:]'))
PROJECT_VERSION := 0.0.0+$(HASH_VERSION)

TARGET_DIR := $(PROJECT_DIR)/target
BINARY_NAME := water-levels
BINARY_DIR := $(TARGET_DIR)/$(BUILD_PROFILE)/$(BINARY_NAME)

DOCKER := DOCKER_BUILDKIT=1 docker
DOCKER_BUILD_ARGS := 
export DOCKER_IMAGE_NAME := chriszen/water-levels
export DOCKER_IMAGE_TAG := $(subst +,-,$(PROJECT_VERSION))
DOCKER_IMAGE := $(DOCKER_IMAGE_NAME):$(DOCKER_IMAGE_TAG)
DOCKERFILE := Dockerfile

FLY_APP_NAME := water-levels

# --[ Build ]---------------------------------------------------------------------

local-all: format clippy build test

build:
	cargo build $(BUILD_PROFILE_ARG) --all-targets

format:
	cargo fmt --all

check-format:
	cargo fmt --all -- --check

clippy:
	cargo clippy $(BUILD_PROFILE_ARG) --all-targets -- -D warnings

test:
	RUST_LOG=$(TEST_RUST_LOG) cargo test $(TEST_PROFILE_ARG) -- --nocapture

run:
	cargo run

clean:
	cargo clean

# --[ Frontend ]---------------------------------------------------------------------

frontend-setup:
	$(MAKE) -C frontend setup

frontend-build:
	$(MAKE) -C frontend webpack WS_URL=$(WS_URL)

# --[ Docker ]---------------------------------------------------------------------

docker-binary-build-linux: build

docker-binary-build-macos:
	@echo "Cross-compiling application ..."
	$(DOCKER) run -ti --rm \
		-w /usr/src \
		-v "$(PROJECT_DIR):/usr/src/:delegated" \
		-v "${HOME}/.cargo/registry:/usr/local/cargo/registry:delegated" \
		rust:$(RUST_VERSION) \
		cargo build $(BUILD_PROFILE_ARG)

docker-binary-copy: docker-binary-build-$(OS)
	@echo "Preparing binary for Docker image ..."
	cp "$(BINARY_DIR)" "$(PROJECT_DIR)"

docker-build: docker-binary-copy frontend-build
	@echo "Building Docker image ..."
	$(DOCKER) build $(DOCKER_BUILD_ARGS) -t "$(DOCKER_IMAGE)" -f "$(DOCKERFILE)" "$(PROJECT_DIR)"

docker-login:
	docker login $(DOCKER_REGISTRY)

docker-push: docker-build
	@echo "Pushing Docker image ..."
	$(DOCKER) push "$(DOCKER_IMAGE)"

docker-run: docker-build
	@echo "Running Docker image ..."
	$(DOCKER) run -ti --rm \
		-v "${HOME}/.kube:/root/.kube:ro" \
		-v "${HOME}/.aws:/root/.aws:ro" \
		-e NOTEBOOKS_NAMESPACE="$(KUBECTL_NAMESPACE)" \
		-p 8000:8000 \
		"$(DOCKER_IMAGE)"

docker-clean:
	@echo "Cleaning Docker temporary files ..."
	rm -f $(PROJECT_DIR)/$(BINARY)

# --[ Fly.io ]---------------------------------------------------------------------

fly-deploy: docker-build
	@echo "Deploying application ..."
	flyctl --app $(FLY_APP_NAME) --image "$(DOCKER_IMAGE)" deploy .

# --[ CI ]---------------------------------------------------------------------

ci-setup:
	@echo "Setting up CI ..."
	$(MAKE) frontend-setup

ci-test: ci-update-project-version
	@echo "Running all the tests ..."
	$(MAKE) check-format clippy test

ci-deploy: ci-update-project-version
	$(MAKE) fly-deploy

ci-update-project-version:
	@echo "Configuring application version to $(PROJECT_VERSION) ..."
	sed -i'' -E "s/^version = \".*\"/version = \"$(PROJECT_VERSION)\"/g" Cargo.toml

# -----------------------------------------------------------------------------

.PHONY: build clippy format check-format test run clean local-all
.PHONY: frontend-setup frontend-build
.PHONY: docker-binary-build-linux docker-binary-build-macos docker-binary-copy docker-build docker-login docker-push docker-run docker-clean
.PHONY: fly-deploy
.PHONY: ci-test ci-deploy ci-update-project-version
