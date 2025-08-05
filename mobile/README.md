# custom xbuild

if you want to use custom build, you neex to install custom version of xbuild available in [xbuild](https://github.com/nikescar/xbuild)

```bash
$ git clone https://github.com/nikescar/xbuild
$ cd xbuild: cargo build --release
$ cp ./target/release/x ~/.cargo/bin/x
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