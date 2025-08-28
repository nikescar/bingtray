## Android

### Requirements

* rust 1.81
* android ndk, sdk, java, llvm, kotlin, and gradle will install with build.sh

### Building
```bash
export ANDROID_NDK_HOME="path/to/ndk"
export ANDROID_HOME="path/to/sdk"

rustup target add aarch64-linux-android
cargo install cargo-ndk

cargo ndk -t arm64-v8a -o app/src/main/jniLibs/ build --release
gradle build
gradle installDebug
```

### Logcat

```bash
# clar logcat
$ adb logcat -c
# full logcat
$ adb logcat -v time -s *:V > fullcat.log
# app specific logcat
$ adb logcat -s BingtrayApp > bingcat.log
```

### Android keystore for github workflow 

Set github secrets on Settings > Security > Secrets and Variables > Actions > Environments > New Secret.

Export keystore to github vars.
```bash
$ base64 release.keystore > release-key-keystore_base64_encoded.txt
# KEYSTORE_BASE64=<ENCODED_KEY>
# STORE_PASSWORD=Test123
# KEY_PASSWORD=Test123
# KEY_ALIAS={crate_name}-release-key
```

### Upload signing key on google play

To upload java singing keystore to google play. You need download upload-encryption key from store.

App Integrity > Change Signing key > Export and upload a key(not using Java Keystore) > Download encryption public key
move it to ```./android/app/```.

```bash
$ cd android/app
$ wget https://www.gstatic.com/play-apps-publisher-rapid/signing-tool/prod/pepk.jar
$ java -jar pepk.jar --keystore=release.keystore --alias=release --output=release-signing-play-generated.zip --include-cert --rsa-aes-encryption --encryption-key-path=encryption_public_key.pem
```
upload created ```release-signing-play-generated.zip``` file.

### Submit app to Google Playstore
1. register google play console
2. internal testing and pass 14 days with published testing
3. add credentials to github repository secrets
4. run github workflow

### Submit app to Amazon Appstore
1. register developer and app and be approved
2. add credentials to github repository secrets
3. run github workflow

https://developer.amazon.com/docs/app-submission/submitting-apps-to-amazon-appstore.html
https://developer.amazon.com/apps-and-games/console/apps/list.html#/

### Publish to Snap
1. register developer and get export credentials
2. add credentials to github repository secrets
3. run github workflow
https://documentation.ubuntu.com/snapcraft/stable/how-to/publishing/publish-a-snap/

```bash
$ snap login
$ snapcraft register $APPNAME
$ snapcraft export-login tmp/snap-export.log
$ export SNAPCRAFT_STORE_CREDENTIALS="your_credentials_here"
$ cargo build --release
$ snapcraft pack --destructive-mode
$ snapcraft upload --release=stable bingtray_0.0.6_amd64.snap
```

### Submit app to Fdroid
1. add metadata to fdroid datarepo
https://gitlab.com/fdroid/fdroiddata

### Publish to Flathub

https://docs.flathub.org/docs/for-app-authors/submission


### Publish to Huawei

https://docs.nhncloud.com/en/Mobile%20Service/IAP/en/console-huawei-guide/

