package pe.nikescar.bingtray;

import android.app.IntentService;
import android.app.WallpaperManager;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.os.Build;
import android.util.Log;

/**
 * Background service to set wallpaper
 */
public class WallpaperSetterService extends IntentService {

    private static final String TAG = "WallpaperSetter";
    private static final String ACTION_SET_WALLPAPER = "pe.nikescar.bingtray.SET_WALLPAPER";
    private static final String EXTRA_IMAGE_BYTES = "image_bytes";

    public WallpaperSetterService() {
        super("WallpaperSetterService");
    }

    /**
     * Start the service to set wallpaper from image bytes
     */
    public static void setWallpaper(Context context, byte[] imageBytes) {
        Intent intent = new Intent(context, WallpaperSetterService.class);
        intent.setAction(ACTION_SET_WALLPAPER);
        intent.putExtra(EXTRA_IMAGE_BYTES, imageBytes);
        context.startService(intent);
    }

    @Override
    protected void onHandleIntent(Intent intent) {
        if (intent == null) {
            return;
        }

        String action = intent.getAction();
        if (ACTION_SET_WALLPAPER.equals(action)) {
            byte[] imageBytes = intent.getByteArrayExtra(EXTRA_IMAGE_BYTES);
            handleSetWallpaper(imageBytes);
        }
    }

    private void handleSetWallpaper(byte[] imageBytes) {
        if (imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "Empty image data");
            return;
        }

        Bitmap bitmap = null;
        try {
            // Decode bitmap from bytes (following reference examples)
            bitmap = BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.length);
            if (bitmap == null) {
                Log.e(TAG, "Failed to decode bitmap from bytes");
                return;
            }

            // Get WallpaperManager
            WallpaperManager wallpaperManager = WallpaperManager.getInstance(this);
            if (wallpaperManager == null) {
                Log.e(TAG, "Failed to get WallpaperManager");
                return;
            }

            // Set wallpaper using bitmap (like reference examples)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                // Set home screen wallpaper only
                wallpaperManager.setBitmap(bitmap, null, true, WallpaperManager.FLAG_SYSTEM);
            } else {
                // Fallback for older Android versions
                wallpaperManager.setBitmap(bitmap);
            }

            Log.i(TAG, "Wallpaper set successfully from service");
        } catch (Exception e) {
            Log.e(TAG, "Failed to set wallpaper: " + e.getMessage(), e);
        } finally {
            // Recycle bitmap to free memory (following reference examples)
            if (bitmap != null && !bitmap.isRecycled()) {
                bitmap.recycle();
            }
        }
    }
}
