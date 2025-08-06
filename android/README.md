# android reports

https://www.shenmeapp.com/appinfo/oqx0Ww7WIt4Tskyf

# F-Droid Metadata

This project includes fastlane metadata for F-Droid submission located in `/fastlane/metadata/android/en-US/`:

- `title.txt` - App name
- `short_description.txt` - Brief description (max 80 chars)
- `full_description.txt` - Detailed app description (max 4000 chars, HTML allowed)
- `images/icon.png` - App icon (recommended: 512x512px)
- `images/phoneScreenshots/` - Phone screenshots
- `changelogs/` - Version-specific changelogs (named by versionCode)

To update for new releases:
1. Add changelog file named with the versionCode (e.g., `changelogs/2.txt`)
2. Update screenshots if UI has changed
3. Ensure metadata is committed before creating release tags

For F-Droid compliance, the project:
- ✅ Uses FLOSS licenses (MIT/Apache-2.0)
- ✅ No Google Play Services dependencies
- ✅ No proprietary analytics/tracking
- ✅ Clean build process with standard tools

# requirements

* rust 1.81
* android ndk, sdk, java, llvm, kotlin, and gradle will install with build.sh

# building
```bash
export ANDROID_NDK_HOME="path/to/ndk"
export ANDROID_HOME="path/to/sdk"

rustup target add aarch64-linux-android
cargo install cargo-ndk

cargo ndk -t arm64-v8a -o app/src/main/jniLibs/ build --release
gradle build
gradle installDebug
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

# android keystore for github workflow 

set github secrets on Settings > Security > Secrets and Variables > Actions > Environments > New Secret

export keystore to github vars.
```bash
$ base64 release.keystore > release-key-keystore_base64_encoded.txt
# KEYSTORE_BASE64=<ENCODED_KEY>
# STORE_PASSWORD=Test123
# KEY_PASSWORD=Test123
# KEY_ALIAS={crate_name}-release-key
```

# upload signing key on google play

upload java singing keystore.

## download upload-encryption key from store

App Integrity > Change Signing key > Export and upload a key(not using Java Keystore) > Download encryption public key
move it to ```./android/app/```.

```bash
$ cd android/app
$ wget https://www.gstatic.com/play-apps-publisher-rapid/signing-tool/prod/pepk.jar
$ java -jar pepk.jar --keystore=release.keystore --alias=release --output=release-signing-play-generated.zip --include-cert --rsa-aes-encryption --encryption-key-path=encryption_public_key.pem
```
upload created ```release-signing-play-generated.zip``` file.

