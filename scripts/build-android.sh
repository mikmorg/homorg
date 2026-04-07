#!/usr/bin/env bash
# build-android.sh — build the homorg_camera Android APK
# Env vars are set inline; no ~/.zshrc changes required.
# Usage: bash scripts/build-android.sh [--release|--debug]
set -euo pipefail

MOUNT="/mnt/homorg-build"
export FLUTTER_ROOT="$MOUNT/flutter"
export ANDROID_HOME="$MOUNT/android-sdk"
export ANDROID_SDK_ROOT="$MOUNT/android-sdk"
export PUB_CACHE="$MOUNT/pub-cache"
export GRADLE_USER_HOME="$MOUNT/gradle"
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk-amd64}"
export PATH="$FLUTTER_ROOT/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

MODE="${1:---release}"

cd "$(dirname "$0")/../mobile"

echo "==> flutter pub get"
flutter pub get

echo "==> flutter build apk $MODE"
flutter build apk "$MODE"

APK_PATH="build/app/outputs/flutter-apk/app-${MODE#--}.apk"
if [[ -f "$APK_PATH" ]]; then
    echo ""
    echo "Build succeeded: mobile/$APK_PATH"
    ls -lh "$APK_PATH"
else
    echo "Build finished — check build/app/outputs/ for APK"
fi
