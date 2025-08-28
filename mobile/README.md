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

### Upload signing key on Google Playstore

To upload java singing keystore to google play. You need download upload-encryption key from store.

App Integrity > Change Signing key > Export and upload a key(not using Java Keystore) > Download encryption public key
move it to ```./android/app/```.

```bash
$ cd android/app
$ wget https://www.gstatic.com/play-apps-publisher-rapid/signing-tool/prod/pepk.jar
$ java -jar pepk.jar --keystore=release.keystore --alias=release --output=release-signing-play-generated.zip --include-cert --rsa-aes-encryption --encryption-key-path=encryption_public_key.pem
```
upload created ```release-signing-play-generated.zip``` file.

### Submit app to Amazon Appstore

1. submit app
2. get app id, appstore client id, appstore client secret
3. test github workflow

https://developer.amazon.com/docs/app-submission/submitting-apps-to-amazon-appstore.html
https://developer.amazon.com/docs/app-submission-api/auth.html

client-id: ${{secrets.AMAZON_APPSTORE_CLIENT_ID}}
client-secret: ${{secrets.AMAZON_APPSTORE_CLIENT_SECRET}}
app-id: ${{ secrets.AMAZON_APPSTORE_APP_ID }}

AMAZON_APPSTORE_CLIENT_ID=amzn1.application-oa2-client.*********************
AMAZON_APPSTORE_CLIENT_SECRET=amzn1.oa2-cs.v1.******************
AMAZON_APPSTORE_APP_ID=amzn1.devportal.mobileapp.0d33f4326f6348218eba1b026204a38f

### Publish to Snap

https://documentation.ubuntu.com/snapcraft/stable/how-to/publishing/publish-a-snap/

### Publish to Flathub

https://docs.flathub.org/docs/for-app-authors/submission
