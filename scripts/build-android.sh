#!/usr/bin/env bash
#
# Build the GLP Atlas Android APK.
#
# Usage:
#   ./scripts/build-android.sh            # debug (fast, unoptimized, debug-signed)
#   ./scripts/build-android.sh --release  # optimized ([profile.release] + R8), debug-key signed
#
# `dx build --platform android` regenerates the Android project on every run. Settings it
# does not take from app/Dioxus.toml are applied here afterwards, then Gradle repackages the
# APK. Idempotent.
#
# Requires: dx (dioxus-cli 0.7.x), Android NDK + SDK, JDK 17, and the env vars
# JAVA_HOME / ANDROID_HOME / ANDROID_NDK_HOME set.
set -euo pipefail

PROFILE=debug
DX_FLAGS=()
GRADLE_TASK=assembleDebug
if [[ "${1:-}" == "--release" ]]; then
  PROFILE=release
  DX_FLAGS=(--release)
  GRADLE_TASK=assembleRelease
elif [[ -n "${1:-}" ]]; then
  echo "unknown argument: $1 (expected --release or nothing)" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Resolve the toolchain from standard env vars.
if [ -z "${ANDROID_HOME:-}" ] && [ -n "${ANDROID_SDK_ROOT:-}" ]; then
  ANDROID_HOME="$ANDROID_SDK_ROOT"
fi
if [ -z "${ANDROID_HOME:-}" ] && [ -d "$HOME/Library/Android/sdk" ]; then
  ANDROID_HOME="$HOME/Library/Android/sdk"   # Android Studio default (macOS)
fi
if [ -z "${ANDROID_HOME:-}" ]; then
  echo "error: set ANDROID_HOME (or ANDROID_SDK_ROOT) to your Android SDK." >&2
  exit 1
fi
export ANDROID_HOME
export ANDROID_SDK_ROOT="$ANDROID_HOME"

if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1 || true)"
fi
if [ -z "${ANDROID_NDK_HOME:-}" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "error: no NDK found. Set ANDROID_NDK_HOME or install one under \$ANDROID_HOME/ndk." >&2
  exit 1
fi
export ANDROID_NDK_HOME

if [ -z "${JAVA_HOME:-}" ] && ! command -v java >/dev/null 2>&1; then
  echo "error: no JDK found. Set JAVA_HOME or put java (17+) on PATH." >&2
  exit 1
fi

# 16 KB LOAD-segment alignment, for Android 15+ devices with a 16 KB page size. dx overrides
# target rustflags from .cargo/config.toml, so this goes through RUSTFLAGS.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-z,max-page-size=16384"

# dx builds from app/, where a relative target directory would resolve against a different
# directory than the paths below, so pin it to the caller's.
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
[[ "$CARGO_TARGET_DIR" == /* ]] || CARGO_TARGET_DIR="$PWD/$CARGO_TARGET_DIR"
export CARGO_TARGET_DIR

echo "Profile: $PROFILE"
echo "ANDROID_HOME=$ANDROID_HOME"
echo "ANDROID_NDK_HOME=$ANDROID_NDK_HOME"
echo "JAVA_HOME=${JAVA_HOME:-<using java on PATH>}"

# 1. Cross-compile the Rust .so and generate the base Android project.
( cd "$ROOT/app" && dx build --platform android "${DX_FLAGS[@]}" )

GEN="$CARGO_TARGET_DIR/dx/glp-atlas/$PROFILE/android/app"
MAIN="$GEN/app/src/main"
GRADLE="$GEN/app/build.gradle.kts"
[ -d "$MAIN" ] || { echo "generated project not found at $MAIN" >&2; exit 1; }

# 2. Drop android:extractNativeLibs. Its absence selects uncompressed, page-aligned native
#    library packaging, which suits 16 KB page-size devices.
MANIFEST="$MAIN/AndroidManifest.xml"
sed 's/ android:extractNativeLibs="[^"]*"//' "$MANIFEST" > "$MANIFEST.tmp" && mv "$MANIFEST.tmp" "$MANIFEST"

# 3. dx writes the launcher label and a placeholder icon from assets with no config hook, so
#    they are replaced here. `dx serve` keeps dx's own.
STRINGS="$MAIN/res/values/strings.xml"
if [ -f "$STRINGS" ]; then
  sed -E 's|(<string name="app_name">)[^<]*(</string>)|\1GLP Atlas\2|' "$STRINGS" > "$STRINGS.tmp" \
    && mv "$STRINGS.tmp" "$STRINGS"
fi

#    These mirror dx's density qualifiers: moving a drawable between qualifiers mid-build
#    leaves Gradle's incremental resource merge unable to resolve it.
cp "$ROOT/app/android/res/drawable/ic_launcher_background.xml" "$MAIN/res/drawable/"
cp "$ROOT/app/android/res/drawable-v24/ic_launcher_foreground.xml" "$MAIN/res/drawable-v24/"

#    The launch frame is a theme attribute with no config hook, and Dioxus.toml's theme is read
#    by `dx serve` too, which builds without this script and would not find a custom style. The
#    theme is swapped in the generated manifest instead, leaving `dx serve` on dx's own.
mkdir -p "$MAIN/res/values"
cp "$ROOT/app/android/res/values/themes.xml" "$MAIN/res/values/"
sed -E 's|android:theme="@style/[^"]*"|android:theme="@style/Theme.Atlas"|' "$MANIFEST" \
  > "$MANIFEST.tmp" && mv "$MANIFEST.tmp" "$MANIFEST"

# 4. Compile against android.jar 36 though the bundled AGP was validated for an older
#    compileSdk.
PROPS="$GEN/gradle.properties"
if [ -f "$PROPS" ] && ! grep -q "suppressUnsupportedCompileSdk" "$PROPS"; then
  printf '\nandroid.suppressUnsupportedCompileSdk=36\n' >> "$PROPS"
fi

# Drop dx's deprecated global BuildConfig default; the module build.gradle already sets
# buildFeatures.buildConfig = true.
if [ -f "$PROPS" ]; then
  sed '/android\.defaults\.buildfeatures\.buildconfig/d' "$PROPS" > "$PROPS.tmp" && mv "$PROPS.tmp" "$PROPS"
fi

# 5. Release-only: sign with the debug key, which keeps the APK sideloadable.
if [[ "$PROFILE" == release ]]; then
  if ! grep -q 'signingConfigs.getByName("debug")' "$GRADLE"; then
    awk '
      /getByName\("release"\)/ { inrelease = 1 }
      inrelease && !inserted && /isMinifyEnabled/ {
        match($0, /^[ \t]*/)
        print substr($0, 1, RLENGTH) "signingConfig = signingConfigs.getByName(\"debug\")"
        inserted = 1
      }
      { print }
    ' "$GRADLE" > "$GRADLE.tmp" && mv "$GRADLE.tmp" "$GRADLE"
  fi
fi

# 6. Repackage the APK with the patched settings, then copy it to a branded name (Gradle
#    names its output after the module, "app-<profile>.apk").
( cd "$GEN" && ./gradlew "$GRADLE_TASK" )

OUT_DIR="$GEN/app/build/outputs/apk/$PROFILE"
APK="$OUT_DIR/glp-atlas-$PROFILE.apk"
cp "$OUT_DIR/app-$PROFILE.apk" "$APK"
echo
echo "Built ($PROFILE): $APK"
