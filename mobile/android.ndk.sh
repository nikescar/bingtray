#!/usr/bin/env bash
# when you need to update the android ndk, run this script
mkdir Android.ndk

if [[ ! -d "$HOME/.android/ndk/android-ndk-r28c" ]]; then
    echo "Installing android ndk..."
    durl="https://dl.google.com/android/repository/android-ndk-r28c-linux.zip"
    pushd "$HOME/Downloads"
    wget --directory-prefix="$HOME/Downloads" "${durl}" 2>&1 1>/dev/null
    unzip android-ndk-r28c-linux.zip 2>&1 1>/dev/null
    mv android-ndk-r28c $HOME/.android/ndk
    popd
fi

export TOOLCHAIN=$HOME/.android/ndk/android-ndk-r28c/toolchains/llvm/prebuilt/linux-x86_64
export CLANG_VERSION=$(ls "$TOOLCHAIN/lib/clang")
export CLANG="$TOOLCHAIN/lib/clang/$CLANG_VERSION"

echo $ANDROID_NDK_LATEST_HOME
echo $CLANG_VERSION

mkdir Android.ndk
echo "android-ndk-r28c" > Android.ndk/latest

cp -r $TOOLCHAIN/sysroot/usr Android.ndk/
cp -r $CLANG/lib/linux/aarch64/* Android.ndk/usr/lib/aarch64-linux-android/
cp -r $CLANG/lib/linux/arm/* Android.ndk/usr/lib/arm-linux-androideabi/
cp -r $CLANG/lib/linux/x86_64/* Android.ndk/usr/lib/x86_64-linux-android/
cp -r $CLANG/lib/linux/i386/* Android.ndk/usr/lib/i686-linux-android/

tar --zstd -cf Android.ndk.tar.zst Android.ndk

# replace existing Android.ndk.tar.zst in cargo cache
rm -rf $HOME/.cache/x/Android.ndk
cp -r Android.ndk $HOME/.cache/x/Android.ndk
cp -r Android.ndk.tar.zst $HOME/.cache/x/download/Android.ndk.tar.zst