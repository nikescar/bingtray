#!/usr/bin/env bash

# extrepo enable debian_official
[[ $(dpkg -l|grep libssl-dev|grep -c "libssl-dev") -lt 1 ]] && apt install libssl-dev squashfs-tools
[[ $(which x|grep -c "/x") -lt 1 ]] && cargo install xbuild
[[ ! -d "$HOME/.local" ]] && mkdir -p $HOME/.local $HOME/Android

# https://static.rust-lang.org/dist/rust-1.88.0-x86_64-unknown-linux-gnu.tar.xz
# tar -xvf rust-1.88.0-x86_64-unknown-linux-gnu.tar.xz
# mv rust-1.88.0-x86_64-unknown-linux-gnu $HOME/.local/

# libpath=("$HOME/.local/jdk-24.0.1/bin" "$HOME/.local/LLVM-20.1.0-Linux-X64/bin" "$HOME/Android/platform-tools" "$HOME/.local/kotlin-native-prebuilt-linux-x86_64-2.2.0/bin" "export PATH=$PATH:$HOME/.local/kotlinc/bin" "$HOME/.local/gradle-8.14.3/bin")
if [[ ! -d "$HOME/.local/jdk-24.0.1/bin" ]]; then
    durl="https://download.java.net/java/GA/jdk24.0.1/24a58e0e276943138bf3e963e6291ac2/9/GPL/openjdk-24.0.1_linux-x64_bin.tar.gz"
    pushd "$HOME/Downloads"
    wget --directory-prefix="$HOME/Downloads" "${durl}"
    tar -zxvf openjdk-24.0.1_linux-x64_bin.tar.gz
    mv jdk-24.0.1/ $HOME/.local/
    popd
fi

if [[ ! -d "$HOME/.local/LLVM-20.1.0-Linux-X64/bin" ]]; then
    durl="https://github.com/llvm/llvm-project/releases/download/llvmorg-20.1.0/LLVM-20.1.0-Linux-X64.tar.xz"
    pushd "$HOME/Downloads"
    wget --directory-prefix="$HOME/Downloads" "${durl}"
    tar -xvf LLVM-20.1.0-Linux-X64.tar.xz
    mv LLVM-20.1.0-Linux-X64 $HOME/.local/
    popd
fi

if [[ ! -d "$HOME/Android/platform-tools" ]]; then
    durl="https://dl.google.com/android/repository/platform-tools-latest-linux.zip"
    pushd "$HOME/Downloads"
    wget --directory-prefix="$HOME/Downloads" "${durl}"
    unzip platform-tools-latest-linux.zip
    mv platform-tools $HOME/Android
    popd
fi

if [[ ! -d "$HOME/.local/kotlin-native-prebuilt-linux-x86_64-2.2.0/bin" ]]; then
    durl="https://github.com/JetBrains/kotlin/releases/download/v2.2.0/kotlin-compiler-2.2.0.zip"
    pushd "$HOME/Downloads"
    wget --directory-prefix="$HOME/Downloads" "${durl}"
    tar -zxvf kotlin-native-prebuilt-linux-x86_64-2.2.0.tar.gz
    mv kotlin-native-prebuilt-linux-x86_64-2.2.0 $HOME/.local
    popd
fi

if [[ ! -d "$HOME/Android/platform-tools" ]]; then
    durl="https://github.com/JetBrains/kotlin/releases/download/v2.2.0/kotlin-native-prebuilt-linux-x86_64-2.2.0.tar.gz"
    pushd "$HOME/Downloads"
    wget --directory-prefix="$HOME/Downloads" "${durl}"
    unzip kotlin-compiler-2.2.0.zip
    mv kotlinc $HOME/.local/
    popd
fi

if [[ ! -d "$HOME/Android/platform-tools" ]]; then
    durl="https://services.gradle.org/distributions/gradle-8.14.3-bin.zip"
    pushd "$HOME/Downloads"
    wget --directory-prefix="$HOME/Downloads" "${durl}"
    unzip gradle-8.14.3-bin.zip
    mv gradle-8.14.3 $HOME/.local
    popd
fi
: <<'END_COMMENT'
export PATH=$PATH:$HOME/.local/jdk-24.0.1/bin
export PATH=$PATH:$HOME/.local/LLVM-20.1.0-Linux-X64/bin
export PATH=$PATH:$HOME/Android/platform-tools
export PATH=$PATH:$HOME/.local/kotlin-native-prebuilt-linux-x86_64-2.2.0/bin
export PATH=$PATH:$HOME/.local/kotlinc/bin
export PATH=$PATH:$HOME/.local/gradle-8.14.3/bin
END_COMMENT

[[ $(grep -c "jdk-24.0.1" $HOME/.bashrc) -lt 1 ]] && echo "export PATH=\$PATH:\$HOME/.local/jdk-24.0.1/bin" >> $HOME/.bashrc
[[ $(grep -c "LLVM-20.1.0-Linux-X64" $HOME/.bashrc) -lt 1 ]] && echo "export PATH=\$PATH:\$HOME/.local/LLVM-20.1.0-Linux-X64/bin" >> $HOME/.bashrc
[[ $(grep -c "platform-tools" $HOME/.bashrc) -lt 1 ]] && echo "export PATH=\$PATH:\$HOME/Android/platform-tools" >> $HOME/.bashrc
[[ $(grep -c "kotlin-native-prebuilt-linux-x86_64-2.2.0" $HOME/.bashrc) -lt 1 ]] && echo "export PATH=\$PATH:\$HOME/.local/kotlin-native-prebuilt-linux-x86_64-2.2.0/bin" >> $HOME/.bashrc
[[ $(grep -c "kotlinc" $HOME/.bashrc) -lt 1 ]] && echo "export PATH=\$PATH:\$HOME/.local/kotlinc/bin" >> $HOME/.bashrc
[[ $(grep -c "gradle-8.14.3" $HOME/.bashrc) -lt 1 ]] && echo "export PATH=\$PATH:\$HOME/.local/gradle-8.14.3/bin" >> $HOME/.bashrc

# https://github.com/aws/aws-lc-rs/blob/main/aws-lc-rs/README.md#Build
# russh -> cmake  >> apt install cmake

x build --arch arm64 --platform android
# adb devices
# adb install ../../target/x/debug/android/Bingtray_android.apk