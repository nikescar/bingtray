package pe.nikescar.bingtray;

import android.app.WallpaperManager;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.util.Log;

public class AndroidServiceWallpaper {
    private static final String TAG = "AndroidServiceWallpaper";
    private WallpaperManager wallpaperManager;
    private Context context;

    public AndroidServiceWallpaper(Context context) {
        this.context = context;
        this.wallpaperManager = WallpaperManager.getInstance(context);
        Log.i(TAG, "AndroidServiceWallpaper initialized");
    }

    /**
     * Set wallpaper from byte array
     * @param imageBytes The image data as byte array
     * @return true if successful, false otherwise
     */
    public boolean setWallpaperFromBytes(byte[] imageBytes) {
        if (imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "Invalid image data: null or empty");
            return false;
        }

        try {
            Log.i(TAG, "Processing " + imageBytes.length + " bytes for wallpaper");
            
            // Decode bitmap from byte array
            Bitmap bitmap = BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.length);
            if (bitmap == null) {
                Log.e(TAG, "Failed to decode bitmap from byte array");
                return false;
            }
            
            Log.i(TAG, "Successfully created bitmap: " + bitmap.getWidth() + "x" + bitmap.getHeight());
            
            // Set the wallpaper
            wallpaperManager.setBitmap(bitmap);
            
            Log.i(TAG, "Wallpaper set successfully");
            
            // Clean up bitmap
            bitmap.recycle();
            Log.i(TAG, "Bitmap recycled");
            
            return true;
            
        } catch (Exception e) {
            Log.e(TAG, "Failed to set wallpaper: " + e.getMessage(), e);
            return false;
        }
    }

    /**
     * JNI wrapper function for Rust integration
     * Called from native code via JNI
     */
    public static boolean setWallpaperFromBytesJNI(Context context, byte[] imageBytes) {
        Log.i(TAG, "JNI setWallpaperFromBytesJNI called with " + 
              (imageBytes != null ? imageBytes.length : 0) + " bytes");
        
        try {
            AndroidServiceWallpaper service = new AndroidServiceWallpaper(context);
            return service.setWallpaperFromBytes(imageBytes);
        } catch (Exception e) {
            Log.e(TAG, "JNI wrapper failed: " + e.getMessage(), e);
            return false;
        }
    }
}