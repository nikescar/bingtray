# Fastlane Metadata for F-Droid

This directory contains the fastlane metadata structure for F-Droid submission of BingTray.

## Directory Structure

```
fastlane/
└── metadata/
    └── android/
        └── en-US/                              # Required fallback locale
            ├── title.txt                       # App name
            ├── short_description.txt           # Brief description (max 80 chars)
            ├── full_description.txt            # Detailed description (max 4000 chars, HTML allowed)
            ├── images/
            │   ├── icon.png                    # App icon (512x512px)
            │   ├── phoneScreenshots/           # Phone screenshots
            │   │   └── 1.png                   # First screenshot
            │   └── README.md                   # Instructions for graphics
            └── changelogs/                     # Version-specific changelogs
                └── 1.txt                       # Changelog for versionCode 1
```

## Files Included

### Required Files ✅
- **title.txt**: "BingTray"
- **short_description.txt**: Cross-platform wallpaper manager description
- **full_description.txt**: Comprehensive app description with features and supported platforms

### Recommended Files ✅
- **images/icon.png**: 512x512px app icon copied from bingtray/resources/logo.png
- **images/phoneScreenshots/1.png**: Mobile screenshot from imgs/mobilescreen.png

### Optional Files ✅
- **changelogs/1.txt**: Initial release changelog for versionCode 1

## Missing Graphics (Optional)

To enhance the F-Droid listing, consider adding:
- **featureGraphic.png**: 1024x500px or 512x250px landscape banner for app description header
- **promoGraphic.png**: 180x120px promotional graphic (rarely used)
- **tvBanner.png**: 1280x720px TV banner (for Android TV)

## F-Droid Compliance

The BingTray project is compliant with F-Droid inclusion policy:

### ✅ License Requirements
- Uses dual MIT/Apache-2.0 licensing (both FLOSS-approved)
- All dependencies are FLOSS

### ✅ Build Requirements  
- No Google Play Services dependencies
- No Firebase/Crashlytics
- No proprietary analytics libraries
- Uses standard build tools (Rust/Cargo, Gradle)

### ✅ Privacy & Security
- No user tracking
- No auto-updates bypassing F-Droid
- Downloads only public Bing wallpapers
- Proper application ID: pe.nikescar.bingtray

### ✅ Source Code
- Publicly available on GitHub
- Maintained and up-to-date
- Clear version tags recommended for releases

## Updating for New Releases

1. **Before creating a release tag:**
   - Add new changelog file: `changelogs/<versionCode>.txt`
   - Update screenshots if UI changed
   - Commit fastlane changes

2. **Version Code Mapping:**
   - Changelog files must be named exactly with the versionCode
   - Example: versionCode=2 → `changelogs/2.txt`
   - Max 500 bytes, plain text (no HTML)

3. **F-Droid picks up metadata from the same tag used to build the APK**

## Resources

- [F-Droid Fastlane Documentation](https://f-droid.org/docs/All_About_Descriptions_Graphics_and_Screenshots/#in-the-apps-source-repository)
- [F-Droid Inclusion Policy](https://f-droid.org/docs/Inclusion_Policy)
- [Fastlane Metadata Structure](https://gitlab.com/-/snippets/1895688)
