** xbuild has broken on android sdk 35. google play required version. but xbuild resource taker has hard problem. moving on to gradle build. **

# custom xbuild

current xbuild aab build is broken, need custom build.

```bash
$ cargo install xbuild --git https://github.com/nikescar/xbuild
```

# building

```bash
export JAVA_HOME=${HOME}/.local/jdk-24.0.1
export ANDROID_HOME=${HOME}/.android
export ANDROID_NDK_HOME=${HOME}/.android/ndk/android-ndk-r28c/
export ANDROID_NDK_ROOT=${HOME}/.android/ndk
export ANDROID_CMDLINE_TOOLS=${HOME}/.android/cmdline-tools/latest/bin
export ANDROID_TOOLS=${HOME}/.android/tools/bin
export ANDROID_PLATFORM_TOOLS=${HOME}/.android/platform-tools

# debug/release|apk/aab|play|arm64/x64 build commands
# debug build for x64,arm64
$ x build --arch x64 --platform android 
$ x build --arch arm64 --platform android
# release build for x64,arm64
$ x build --arch x64 --platform android --release
$ x build --arch arm64 --platform android --release
# release aab build for x64,arm64
$ x build --arch x64 --platform android --release --format aab
$ x build --arch arm64 --platform android --release --format aab
# release for playstore for x64,arm64
$ x build --arch x64 --platform android --release --store play
$ x build --arch arm64 --platform android --release --store play
```

# installing

```bash
# run build.sh
$ ./build.sh
```

# logcat

```bash
# clar logcat
$ adb logcat -c
# full logcat
$ adb logcat -v time -s *:V > fullcat.log
# app specific logcat
$ adb logcat -s BingtrayApp > bingcat.log
```

# github workflow google keystore

set github secrets on Settings > Security > Secrets and Variables > Actions > Environments > New Secret

export keystore to github vars.
```bash
$ base64 {crate_name}-release-key.keystore > {crate_name}-release-key-keystore_base64_encoded.txt
# echo "$ENCODED_STRING" | base64 -d > {crate_name}-release-key.keystore.jks
# KEYSTORE_BASE64=<ENCODED_KEY>
# STORE_PASSWORD=Test123
# KEY_PASSWORD=Test123
# KEY_ALIAS={crate_name}-release-key
```

# google play

upload java singing keystore.

## download upload encryption key from store

App Integrity > Change Signing key > Export and upload a key(not using Java Keystore) > Download encryption public key
move it to ```target/x/release/android/keys```.

```bash
$ cd target/s/release/android
$ wget https://www.gstatic.com/play-apps-publisher-rapid/signing-tool/prod/pepk.jar
$ java -jar pepk.jar --keystore=keys/mobile-release-key.keystore --alias=mobile-release-key --output=mobile-signing-play-generated.zip --include-cert --rsa-aes-encryption --encryption-key-path=keys/encryption_public_key.pem
```
upload created ```mobile-signing-play-generated.zip``` file.

