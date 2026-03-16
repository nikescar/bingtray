#!/usr/bin/env bash
# Fast Rust-only rebuild script for development
# Recompiles Rust library, unpacks existing APK, replaces .so file, and repacks without gradle

set -e

# Configuration
BASE_APK="${BASE_APK:-app/build/outputs/apk/debug/app-arm64-v8a-debug.apk}"
KEYSTORE="${KEYSTORE:-app/release.keystore}"
KEY_ALIAS="${KEY_ALIAS:-bingtray_key}"
WORK_DIR="build_temp"
OUTPUT_APK="app/build/outputs/apk/debug/bingtray-rust-updated.apk"

if [[ ! -f "$HOME/.cargo/bin/rustc" ]]; then
    echo "Installing rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
    rustup default nightly
fi
source $HOME/.cargo/env

if [[ ! -d "$HOME/.android/ndk/android-ndk-r28c" ]]; then
    echo "Installing android ndk..."
    durl="https://dl.google.com/android/repository/android-ndk-r28c-linux.zip"
    pushd "$HOME/Downloads"
    [[ ! -f "android-ndk-r28c-linux.zip" ]] && wget --directory-prefix="$HOME/Downloads" "${durl}" 2>&1 1>/dev/null
    unzip android-ndk-r28c-linux.zip 2>&1 1>/dev/null
    mkdir -p $HOME/.android/ndk
    mv android-ndk-r28c $HOME/.android/ndk
    popd
fi

rustup target add aarch64-linux-android
# rustup target add armv7-linux-androideabi
# rustup target add i686-linux-android
# rustup target add x86_64-linux-android
cargo install cargo-ndk

# Set required environment variables
export ANDROID_NDK_HOME=$HOME/.android/ndk/android-ndk-r28c/
export NDK_HOME=$HOME/.android/ndk/android-ndk-r28c/
export ANDROID_HOME=$HOME/.android
export RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none"

# Set version info
timestamp=$(date +%y%m%d%H%M)
export APPLICATION_VERSION_CODE=${timestamp:0:-1}
export APPLICATION_VERSION_NAME=$(grep -m1 "^version = " ../Cargo.toml | cut -d' ' -f3 | tr -d '"')

# Build only arm64-v8a for faster development (most common architecture)
echo "Building Rust library for arm64-v8a..."
cargo ndk -t arm64-v8a -o app/src/main/jniLibs/ build --release --lib

# Copy libc++_shared.so
echo "Copying libc++_shared.so..."
cp "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" \
   app/src/main/jniLibs/arm64-v8a/ || echo "Warning: Failed to copy libc++_shared.so"

# Check if base APK exists
if [[ ! -f "$BASE_APK" ]]; then
    echo "Error: Base APK not found at $BASE_APK"
    echo "Run a full build first or set BASE_APK environment variable"
    exit 1
fi

# Clean up previous work directory
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

# Step 1: Unpack the APK
echo "Unpacking APK: $BASE_APK"
apktool d "$BASE_APK" -o "$WORK_DIR/extracted" -f

# Step 2: Replace the .so files
echo "Replacing .so files..."
cp -v app/src/main/jniLibs/arm64-v8a/*.so "$WORK_DIR/extracted/lib/arm64-v8a/"

# Step 3: Repackage the APK
echo "Repackaging APK..."
apktool b "$WORK_DIR/extracted" -o "$WORK_DIR/unsigned.apk"

# Step 4: Generate keystore if it doesn't exist
if [[ ! -f "$KEYSTORE" ]]; then
    echo "Keystore not found. Generating new keystore at $KEYSTORE"
    keytool -genkey -v -keystore "$KEYSTORE" -alias "$KEY_ALIAS" \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass android -keypass android \
        -dname "CN=Developer, OU=Dev, O=Bingtray, L=Unknown, S=Unknown, C=US"
fi

# Step 5: Sign the APK
echo "Signing APK..."
jarsigner -verbose -sigalg SHA256withRSA -digestalg SHA-256 \
    -keystore "$KEYSTORE" -storepass android -keypass android \
    "$WORK_DIR/unsigned.apk" "$KEY_ALIAS"

# Step 6: Zipalign
echo "Optimizing with zipalign..."
zipalign -v 4 "$WORK_DIR/unsigned.apk" "$OUTPUT_APK"

echo ""
echo "✓ Done! APK ready at: $OUTPUT_APK"
echo ""
echo "Install with: adb install -r $OUTPUT_APK"
