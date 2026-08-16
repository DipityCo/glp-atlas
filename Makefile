# GLP Atlas developer tasks. Run `make` (or `make help`) to list targets.
#
# Android targets shell out to scripts/build-android.sh, which reads JAVA_HOME,
# ANDROID_HOME and ANDROID_NDK_HOME.

APP         := app
DEBUG_APK   := app/target/dx/glp-atlas/debug/android/app/app/build/outputs/apk/debug/glp-atlas-debug.apk
RELEASE_APK := app/target/dx/glp-atlas/release/android/app/app/build/outputs/apk/release/glp-atlas-release.apk

# 16 KB LOAD-segment alignment, for Android 15+ devices with a 16 KB page size. dx overrides
# target rustflags from .cargo/config.toml, so this goes through RUSTFLAGS.
ANDROID_RUSTFLAGS := $${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384

.DEFAULT_GOAL := help
.PHONY: help check check-wasm check-android test fmt fmt-check clippy shellcheck web hotpatch apk apk-release install install-release logcat ci clean

help: ## List available targets
	@grep -hE '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

check: check-wasm check-android ## Type-check both targets the app ships to

check-wasm: ## Type-check the web build
	cargo check --manifest-path $(APP)/Cargo.toml --target wasm32-unknown-unknown \
		--no-default-features --features web

check-android: ## Type-check the shipping Android target
	cargo check --manifest-path $(APP)/Cargo.toml --target aarch64-linux-android

test: ## Run the unit tests
	cargo test --manifest-path $(APP)/Cargo.toml

fmt: ## Format all Rust code
	cargo fmt --manifest-path $(APP)/Cargo.toml

fmt-check: ## Verify formatting without modifying files
	cargo fmt --manifest-path $(APP)/Cargo.toml --check

clippy: ## Lint with clippy
	cargo clippy --manifest-path $(APP)/Cargo.toml --all-targets -- -D warnings

# Info-level findings are advisory; warnings and above are not.
shellcheck: ## Lint the shell scripts
	shellcheck --severity=warning scripts/*.sh

web: ## Serve the UI in a browser, the fast loop for working on the star field
	cd $(APP) && dx serve --platform web

# dx generates, installs and launches its own Android project, reading app/Dioxus.toml but
# not this file, so the launcher label and icon are dx's defaults. Hot-patching is
# experimental upstream. DEVICE=<name> picks between attached devices.
hotpatch: ## Live-patch Rust changes into the app on a connected device (DEVICE=<name> optional)
	cd $(APP) && RUSTFLAGS="$(ANDROID_RUSTFLAGS)" \
		dx serve --platform android --hot-patch $(if $(DEVICE),--device $(DEVICE))

apk: ## Build the debug APK
	./scripts/build-android.sh

apk-release: ## Build the optimized, R8-minified release APK (debug-signed)
	./scripts/build-android.sh --release

install: apk ## Build + install the debug APK on a connected device
	adb install -r "$(DEBUG_APK)"

install-release: apk-release ## Build + install the release APK on a connected device
	adb install -r "$(RELEASE_APK)"

logcat: ## Tail the app's logs
	adb logcat -c && adb logcat RustStdoutStderr:* AndroidRuntime:E *:S

ci: fmt-check clippy shellcheck check test ## Run everything CI gates on

clean: ## Remove all build artifacts
	cargo clean --manifest-path $(APP)/Cargo.toml
