#!/usr/bin/env bash
# build-android.sh — build the Homorg Android APK
# Env vars are set inline; no ~/.zshrc changes required.
# Usage: bash scripts/build-android.sh [--release|--debug]
set -euo pipefail

MOUNT="/scratch/homorg-build"
export FLUTTER_ROOT="$MOUNT/flutter"
export ANDROID_HOME="$MOUNT/android-sdk"
export ANDROID_SDK_ROOT="$MOUNT/android-sdk"
export PUB_CACHE="$MOUNT/pub-cache"
export GRADLE_USER_HOME="$MOUNT/gradle"
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
export PATH="$FLUTTER_ROOT/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

MODE="${1:---release}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_DIR/mobile"

echo "==> flutter pub get"
flutter pub get

echo "==> flutter build apk $MODE"
flutter build apk "$MODE"

APK_PATH="build/app/outputs/flutter-apk/app-${MODE#--}.apk"
if [[ -f "$APK_PATH" ]]; then
    echo ""
    echo "Build succeeded: mobile/$APK_PATH"
    ls -lh "$APK_PATH"

    # Publish the release APK to the backend's downloads dir so the web UI can
    # serve it at /downloads/homorg.apk.
    if [[ "$MODE" == "--release" ]]; then
        DOWNLOADS_DIR="$REPO_DIR/downloads"
        mkdir -p "$DOWNLOADS_DIR"
        cp "$APK_PATH" "$DOWNLOADS_DIR/homorg.apk"
        echo "Published to $DOWNLOADS_DIR/homorg.apk"
    fi
else
    echo "Build finished — check build/app/outputs/ for APK"
fi
