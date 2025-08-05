# custom xbuild

current xbuild aab build is broken, need custom build.

```bash
$ cargo install xbuild --git https://github.com/nikescar/xbuild
```

# building

```bash
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
$ openssl base64 < {crate_name}-release-key.keystore | tr -d '\n' | tee {crate_name}-release-key-keystore_base64_encoded.txt
# KEYSTORE_BASE64=<ENCODED_KEY>
# STORE_PASSWORD=Test123
# KEY_PASSWORD=Test123
# KEY_ALIAS={crate_name}-release-key
```


