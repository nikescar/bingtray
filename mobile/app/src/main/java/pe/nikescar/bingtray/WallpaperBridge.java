package pe.nikescar.bingtray;

import android.app.Activity;
import android.app.WallpaperManager;
import android.content.ComponentName;
import android.content.ContentResolver;
import android.content.ContentValues;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.media.MediaScannerConnection;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.MediaStore;
import android.util.Log;

import androidx.core.content.FileProvider;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStream;

/**
 * Bridge between Rust and Java for wallpaper operations
 * Provides static methods for JNI calls
 */
public class WallpaperBridge {

    private static final String TAG = "WallpaperBridge";
    private static volatile Context sContext = null;
    private static volatile Activity sActivity = null;

    /**
     * Initialize the bridge with application context
     * Must be called from main activity before using other methods
     */
    public static void init(Context context) {
        if (context != null) {
            sContext = context.getApplicationContext();
            // Store activity reference if passed
            if (context instanceof Activity) {
                sActivity = (Activity) context;
                Log.i(TAG, "WallpaperBridge initialized with Activity context");
            } else {
                Log.i(TAG, "WallpaperBridge initialized with Application context");
            }
        } else {
            Log.e(TAG, "WallpaperBridge init: null context");
        }
    }

    /**
     * Set static wallpaper using WallpaperManager
     */
    public static boolean setStaticWallpaperFromBytes(byte[] imageBytes) {
        if (sContext == null) {
            Log.e(TAG, "setStaticWallpaperFromBytes: context not initialized");
            return false;
        }

        if (imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "setStaticWallpaperFromBytes: empty image data");
            return false;
        }

        try {
            // Use background service to set wallpaper in separate process
            WallpaperSetterService.setWallpaper(sContext, imageBytes);
            Log.i(TAG, "Static wallpaper setting initiated via background service");
            return true;
        } catch (Exception e) {
            Log.e(TAG, "setStaticWallpaperFromBytes failed", e);
            return false;
        }
    }

    /**
     * Set wallpaper from image bytes using the live wallpaper service
     */
    public static boolean setWallpaperFromBytes(byte[] imageBytes) {
        try {
            boolean result = ImageLiveWallpaper.setWallpaperFromBytes(imageBytes);
            if (result) {
                Log.i(TAG, "Wallpaper updated successfully");
            } else {
                Log.e(TAG, "Failed to update wallpaper");
            }
            return result;
        } catch (Exception e) {
            Log.e(TAG, "setWallpaperFromBytes failed", e);
            return false;
        }
    }

    /**
     * Save wallpaper image for later use (e.g., when opening the picker)
     * This saves the image even if the live wallpaper service is not active
     */
    public static boolean saveWallpaperImage(byte[] imageBytes) {
        if (sContext == null) {
            Log.e(TAG, "saveWallpaperImage: context not initialized");
            return false;
        }

        if (imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "saveWallpaperImage: empty image data");
            return false;
        }

        try {
            File imageFile = new File(sContext.getFilesDir(), "wallpaper.jpg");
            FileOutputStream fos = new FileOutputStream(imageFile);
            fos.write(imageBytes);
            fos.close();
            Log.i(TAG, "Saved wallpaper image to: " + imageFile.getAbsolutePath());
            return true;
        } catch (IOException e) {
            Log.e(TAG, "Failed to save wallpaper image", e);
            return false;
        }
    }

    /**
     * Copy wallpaper image to public Pictures directory so media selector can see it
     * Returns true if successful
     */
    public static boolean copyWallpaperToPublicPictures() {
        if (sContext == null) {
            Log.e(TAG, "copyWallpaperToPublicPictures: context not initialized");
            return false;
        }

        try {
            // Get the saved wallpaper file
            File sourceFile = ImageLiveWallpaper.getWallpaperFile(sContext);
            if (sourceFile == null || !sourceFile.exists()) {
                Log.w(TAG, "No wallpaper file found to copy");
                return false;
            }

            // Read the image bytes
            byte[] imageBytes = new byte[(int) sourceFile.length()];
            try (FileInputStream fis = new FileInputStream(sourceFile)) {
                fis.read(imageBytes);
            }

            // Use MediaStore for Android 10+ (API 29+) to save to public Pictures
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                ContentResolver resolver = sContext.getContentResolver();
                ContentValues contentValues = new ContentValues();
                contentValues.put(MediaStore.MediaColumns.DISPLAY_NAME, "Bingtray_Wallpaper.jpg");
                contentValues.put(MediaStore.MediaColumns.MIME_TYPE, "image/jpeg");
                contentValues.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_PICTURES + "/Bingtray");

                Uri imageUri = resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, contentValues);
                if (imageUri == null) {
                    Log.e(TAG, "Failed to create MediaStore entry");
                    return false;
                }

                try (OutputStream out = resolver.openOutputStream(imageUri)) {
                    if (out != null) {
                        out.write(imageBytes);
                        Log.i(TAG, "Copied wallpaper to MediaStore: " + imageUri);
                        return true;
                    }
                }
            } else {
                // For older Android versions, copy to Pictures directory and scan
                File picturesDir = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_PICTURES);
                File bingtrayDir = new File(picturesDir, "Bingtray");
                if (!bingtrayDir.exists()) {
                    bingtrayDir.mkdirs();
                }

                File destFile = new File(bingtrayDir, "Bingtray_Wallpaper.jpg");
                try (FileOutputStream fos = new FileOutputStream(destFile)) {
                    fos.write(imageBytes);
                }

                // Notify MediaScanner so the file appears in gallery
                MediaScannerConnection.scanFile(
                    sContext,
                    new String[]{destFile.getAbsolutePath()},
                    new String[]{"image/jpeg"},
                    (path, uri) -> Log.i(TAG, "MediaScanner finished scanning: " + path)
                );

                Log.i(TAG, "Copied wallpaper to: " + destFile.getAbsolutePath());
                return true;
            }
        } catch (Exception e) {
            Log.e(TAG, "Failed to copy wallpaper to public Pictures", e);
            return false;
        }

        return false;
    }

    /**
     * Check if Bingtray wallpaper is currently active
     */
    public static boolean isWallpaperActive() {
        if (sContext == null) {
            Log.e(TAG, "isWallpaperActive: context not initialized");
            return false;
        }

        try {
            WallpaperManager wm = WallpaperManager.getInstance(sContext);
            if (wm == null) {
                return false;
            }

            // Get wallpaper info (API 24+)
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.N) {
                android.app.WallpaperInfo info = wm.getWallpaperInfo();
                if (info != null) {
                    ComponentName serviceName = info.getComponent();
                    String packageName = serviceName.getPackageName();
                    String className = serviceName.getClassName();

                    boolean isActive = packageName.equals(sContext.getPackageName()) &&
                                     className.equals(ImageLiveWallpaper.class.getName());

                    Log.d(TAG, "Wallpaper active: " + isActive + " (service: " + className + ")");
                    return isActive;
                }
            }

            return false;
        } catch (Exception e) {
            Log.e(TAG, "isWallpaperActive failed", e);
            return false;
        }
    }

    /**
     * Copy wallpaper file to external app directory and get a shareable content URI
     * Returns the content:// URI of the copied file, or null if failed
     */
    private static Uri prepareWallpaperForPicker() {
        if (sContext == null) {
            Log.e(TAG, "prepareWallpaperForPicker: context not initialized");
            return null;
        }

        try {
            // Get the saved wallpaper file
            File sourceFile = ImageLiveWallpaper.getWallpaperFile(sContext);
            if (sourceFile == null || !sourceFile.exists()) {
                Log.w(TAG, "No wallpaper file found to prepare");
                return null;
            }

            // Use app's external files directory (no permissions needed)
            File externalDir = sContext.getExternalFilesDir(Environment.DIRECTORY_PICTURES);
            if (externalDir == null) {
                Log.e(TAG, "External files directory not available");
                return null;
            }

            if (!externalDir.exists()) {
                externalDir.mkdirs();
            }

            // Copy file to external app directory
            File destFile = new File(externalDir, "current_wallpaper.jpg");
            try (FileInputStream in = new FileInputStream(sourceFile);
                 FileOutputStream out = new FileOutputStream(destFile)) {
                byte[] buffer = new byte[8192];
                int bytesRead;
                while ((bytesRead = in.read(buffer)) != -1) {
                    out.write(buffer, 0, bytesRead);
                }
            }

            Log.i(TAG, "Copied wallpaper to: " + destFile.getAbsolutePath());

            // Use FileProvider to get a content:// URI (required for Android 7.0+)
            Uri contentUri = FileProvider.getUriForFile(
                    sContext,
                    "pe.nikescar.bingtray.fileprovider",
                    destFile
            );
            Log.i(TAG, "Created content URI: " + contentUri);
            return contentUri;
        } catch (Exception e) {
            Log.e(TAG, "Failed to prepare wallpaper for picker", e);
            return null;
        }
    }

    /**
     * Open the system wallpaper picker to set Bingtray as live wallpaper
     */
    public static boolean openWallpaperPicker() {
        // Prefer Activity context for launching activities
        Context contextToUse = sActivity != null ? sActivity : sContext;

        if (contextToUse == null) {
            Log.e(TAG, "openWallpaperPicker: context not initialized");
            return false;
        }

        // Try to open the live wallpaper picker first
        Intent liveWallpaperIntent = new Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER);
        liveWallpaperIntent.putExtra(WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT,
                new ComponentName(sContext, ImageLiveWallpaper.class));
        // Only add NEW_TASK flag if using application context
        if (!(contextToUse instanceof Activity)) {
            liveWallpaperIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        }

        // Check if the device can handle the live wallpaper intent
        if (liveWallpaperIntent.resolveActivity(contextToUse.getPackageManager()) != null) {
            try {
                contextToUse.startActivity(liveWallpaperIntent);
                Log.i(TAG, "Opened wallpaper picker");
                return true;
            } catch (Exception e) {
                Log.e(TAG, "Failed to open wallpaper picker", e);
            }
        } else {
            Log.w(TAG, "No Activity found for CHANGE_LIVE_WALLPAPER intent, using fallback");
        }

        // Fallback 1: Try CROP_AND_SET_WALLPAPER action to directly preview wallpaper
        Uri wallpaperUri = prepareWallpaperForPicker();
        if (wallpaperUri != null) {
            try {
                Intent intent = new Intent("android.service.wallpaper.CROP_AND_SET_WALLPAPER");
                intent.setData(wallpaperUri);
                intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                // Only add NEW_TASK flag if using application context
                if (!(contextToUse instanceof Activity)) {
                    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                }
                contextToUse.startActivity(intent);
                Log.i(TAG, "Opened wallpaper crop/preview screen with image");
                return true;
            } catch (Exception e) {
                Log.d(TAG, "CROP_AND_SET_WALLPAPER not available, trying SET_AS");

                // Fallback 2: Try SET_AS action
                try {
                    Intent setAsIntent = new Intent("android.intent.action.SET_AS");
                    setAsIntent.setDataAndType(wallpaperUri, "image/*");
                    setAsIntent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    // Only add NEW_TASK flag if using application context
                    if (!(contextToUse instanceof Activity)) {
                        setAsIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                    }
                    contextToUse.startActivity(setAsIntent);
                    Log.i(TAG, "Opened wallpaper setter with image");
                    return true;
                } catch (Exception e2) {
                    Log.d(TAG, "SET_AS not available, falling back to media selector");
                }
            }
        }

        // Fallback: open live wallpaper chooser so user can select Bingtray.
        // Prefer this over ACTION_SET_WALLPAPER: if the user picks a static photo via
        // ACTION_SET_WALLPAPER, Android immediately triggers Material You dynamic-color
        // overlay updates (ApplicationInfo.scheduleApplicationInfoChanged), which forces
        // an Activity Destroy while eframe is recreating its GL surface, causing the
        // Rust render thread to hang and an ANR. Live wallpaper selection defers color
        // extraction so the app is in a stable render state when the change lands.
        try {
            Intent intent = new Intent(WallpaperManager.ACTION_LIVE_WALLPAPER_CHOOSER);
            if (!(contextToUse instanceof Activity)) {
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            }
            contextToUse.startActivity(intent);
            Log.i(TAG, "Opened live wallpaper chooser");
            return true;
        } catch (Exception e) {
            Log.d(TAG, "ACTION_LIVE_WALLPAPER_CHOOSER not available, falling back to wallpaper selector");
        }

        // Last resort: generic wallpaper selector. Only reached on devices that have no
        // live-wallpaper chooser (e.g. bare AOSP/QEMU emulators), which also typically
        // lack Material You, so the static-wallpaper → dynamic-color crash risk is low.
        try {
            Intent intent = new Intent(Intent.ACTION_SET_WALLPAPER);
            if (!(contextToUse instanceof Activity)) {
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            }
            contextToUse.startActivity(intent);
            Log.i(TAG, "Opened wallpaper media selector (last resort)");
            return true;
        } catch (Exception e) {
            Log.e(TAG, "Failed to open any wallpaper picker", e);
            return false;
        }
    }

    /**
     * Get the package name for the wallpaper service
     */
    public static String getServicePackageName() {
        if (sContext == null) {
            return "";
        }
        return sContext.getPackageName();
    }

    /**
     * Get the class name for the wallpaper service
     */
    public static String getServiceClassName() {
        return ImageLiveWallpaper.class.getName();
    }

    /**
     * Test method 1: Try to open live wallpaper picker directly
     */
    public static boolean testLiveWallpaperPicker() {
        Context contextToUse = sActivity != null ? sActivity : sContext;
        if (contextToUse == null) {
            Log.e(TAG, "testLiveWallpaperPicker: context not initialized");
            return false;
        }

        try {
            Intent intent = new Intent(WallpaperManager.ACTION_CHANGE_LIVE_WALLPAPER);
            intent.putExtra(WallpaperManager.EXTRA_LIVE_WALLPAPER_COMPONENT,
                    new ComponentName(sContext, ImageLiveWallpaper.class));
            if (!(contextToUse instanceof Activity)) {
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            }

            if (intent.resolveActivity(contextToUse.getPackageManager()) != null) {
                contextToUse.startActivity(intent);
                Log.i(TAG, "Test: Opened live wallpaper picker");
                return true;
            } else {
                Log.w(TAG, "Test: No Activity for CHANGE_LIVE_WALLPAPER");
                return false;
            }
        } catch (Exception e) {
            Log.e(TAG, "Test: Failed to open live wallpaper picker", e);
            return false;
        }
    }

    /**
     * Test method 2: Try CROP_AND_SET_WALLPAPER action
     */
    public static boolean testCropAndSetWallpaper() {
        Context contextToUse = sActivity != null ? sActivity : sContext;
        if (contextToUse == null) {
            Log.e(TAG, "testCropAndSetWallpaper: context not initialized");
            return false;
        }

        Uri wallpaperUri = prepareWallpaperForPicker();
        if (wallpaperUri == null) {
            Log.w(TAG, "Test: No wallpaper image available");
            return false;
        }

        try {
            Intent intent = new Intent("android.service.wallpaper.CROP_AND_SET_WALLPAPER");
            intent.setData(wallpaperUri);
            intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            if (!(contextToUse instanceof Activity)) {
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            }
            contextToUse.startActivity(intent);
            Log.i(TAG, "Test: Opened CROP_AND_SET_WALLPAPER");
            return true;
        } catch (Exception e) {
            Log.e(TAG, "Test: Failed to open CROP_AND_SET_WALLPAPER", e);
            return false;
        }
    }

    /**
     * Test method 3: Try to open wallpaper media selector (no image provided)
     */
    public static boolean testWallpaperMediaSelector() {
        Context contextToUse = sActivity != null ? sActivity : sContext;
        if (contextToUse == null) {
            Log.e(TAG, "testWallpaperMediaSelector: context not initialized");
            return false;
        }

        try {
            Intent intent = new Intent(Intent.ACTION_SET_WALLPAPER);
            if (!(contextToUse instanceof Activity)) {
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            }
            contextToUse.startActivity(intent);
            Log.i(TAG, "Test: Opened wallpaper media selector");
            return true;
        } catch (Exception e) {
            Log.e(TAG, "Test: Failed to open wallpaper media selector", e);
            return false;
        }
    }
}
