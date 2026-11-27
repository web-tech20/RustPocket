#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENGINE_DIR="$ROOT_DIR/engine"
ANDROID_JNILIBS="$ROOT_DIR/app/android/app/src/main/jniLibs"
IOS_LIBS="$ROOT_DIR/app/ios/Runner/Frameworks"

TARGET="${1:-all}"

build_linux() {
  echo "Building native engine for Linux..."
  cargo build --manifest-path "$ENGINE_DIR/Cargo.toml" --release
}

build_android() {
  if ! command -v cargo-ndk >/dev/null 2>&1; then
    echo "cargo-ndk non installé. Installez-le avec : cargo install cargo-ndk"
    echo "Et ajoutez les cibles : rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android"
    exit 1
  fi
  echo "Building native engine for Android (arm64-v8a, armeabi-v7a, x86_64)..."
  mkdir -p "$ANDROID_JNILIBS"
  cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    -o "$ANDROID_JNILIBS" \
    --manifest-path "$ENGINE_DIR/Cargo.toml" \
    build --release
  echo "Android jniLibs prêtes dans $ANDROID_JNILIBS"
}

build_ios() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "La compilation iOS nécessite macOS + Xcode."
    return 0
  fi
  echo "Building native engine for iOS (aarch64-apple-ios, aarch64-apple-ios-sim)..."
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim
  cargo build --manifest-path "$ENGINE_DIR/Cargo.toml" --release --target aarch64-apple-ios
  cargo build --manifest-path "$ENGINE_DIR/Cargo.toml" --release --target aarch64-apple-ios-sim
  mkdir -p "$IOS_LIBS"
  cp "$ENGINE_DIR/target/aarch64-apple-ios/release/librustpocket_engine.a" "$IOS_LIBS/librustpocket_engine_device.a"
  cp "$ENGINE_DIR/target/aarch64-apple-ios-sim/release/librustpocket_engine.a" "$IOS_LIBS/librustpocket_engine_sim.a"
  echo "iOS staticlibs prêtes dans $IOS_LIBS"
}

case "$TARGET" in
  linux) build_linux ;;
  android) build_android ;;
  ios) build_ios ;;
  all)
    build_linux
    if command -v cargo-ndk >/dev/null 2>&1; then
      build_android
    else
      echo "Note: cargo-ndk absent, compilation Android ignorée."
    fi
    build_ios
    ;;
  *)
    echo "Usage: $0 [all|linux|android|ios]"
    exit 1
    ;;
esac
