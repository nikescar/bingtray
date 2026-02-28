package pe.nikescar.bingtray;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Matrix;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.RectF;
import android.service.wallpaper.WallpaperService;
import android.util.Log;
import android.view.SurfaceHolder;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;

/**
 * Simple image-based live wallpaper service
 * Based on VideoLiveWallpaper reference implementation
 */
public class ImageLiveWallpaper extends WallpaperService {

    private static final String TAG = "ImageLiveWallpaper";
    private static final String IMAGE_FILE_NAME = "wallpaper.jpg";

    // Volatile for thread-safe access
    private static volatile byte[] sPendingImageBytes = null;
    private static volatile ImageEngine sActiveEngine = null;

    /**
     * Set wallpaper from image bytes
     * This method can be called from any thread
     */
    public static boolean setWallpaperFromBytes(byte[] imageBytes) {
        if (imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "setWallpaperFromBytes: empty image data");
            return false;
        }

        try {
            // Store image bytes for the engine to pick up
            sPendingImageBytes = imageBytes;

            // If engine is active, update it immediately
            if (sActiveEngine != null) {
                sActiveEngine.updateWallpaper(imageBytes);
                return true;
            } else {
                Log.w(TAG, "No active engine - image will be shown when wallpaper service starts");
                return true;
            }
        } catch (Exception e) {
            Log.e(TAG, "setWallpaperFromBytes failed", e);
            return false;
        }
    }

    /**
     * Get the path to the saved wallpaper image file
     * Returns null if no file exists
     */
    public static File getWallpaperFile(android.content.Context context) {
        File imageFile = new File(context.getFilesDir(), IMAGE_FILE_NAME);
        if (imageFile.exists()) {
            return imageFile;
        }
        return null;
    }

    @Override
    public Engine onCreateEngine() {
        Log.i(TAG, "Creating new ImageEngine");
        return new ImageEngine();
    }

    class ImageEngine extends Engine {
        private Bitmap currentBitmap;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
        private boolean visible = true;
        private final Object bitmapLock = new Object();

        @Override
        public void onCreate(SurfaceHolder surfaceHolder) {
            super.onCreate(surfaceHolder);
            sActiveEngine = this;
            Log.i(TAG, "ImageEngine created");

            // Load any pending image or the saved image file
            if (sPendingImageBytes != null) {
                loadBitmapFromBytes(sPendingImageBytes);
                sPendingImageBytes = null;
            } else {
                loadBitmapFromFile();
            }
        }

        @Override
        public void onSurfaceCreated(SurfaceHolder holder) {
            super.onSurfaceCreated(holder);
            Log.i(TAG, "Surface created");
            drawFrame();
        }

        @Override
        public void onSurfaceChanged(SurfaceHolder holder, int format, int width, int height) {
            super.onSurfaceChanged(holder, format, width, height);
            Log.i(TAG, "Surface changed: " + width + "x" + height);
            drawFrame();
        }

        @Override
        public void onVisibilityChanged(boolean visible) {
            this.visible = visible;
            if (visible) {
                drawFrame();
            }
        }

        @Override
        public void onSurfaceDestroyed(SurfaceHolder holder) {
            super.onSurfaceDestroyed(holder);
            Log.i(TAG, "Surface destroyed");
        }

        @Override
        public void onDestroy() {
            super.onDestroy();
            sActiveEngine = null;
            synchronized (bitmapLock) {
                if (currentBitmap != null && !currentBitmap.isRecycled()) {
                    currentBitmap.recycle();
                    currentBitmap = null;
                }
            }
            Log.i(TAG, "ImageEngine destroyed");
        }

        /**
         * Update wallpaper with new image bytes
         * Called from setWallpaperFromBytes()
         */
        public void updateWallpaper(byte[] imageBytes) {
            loadBitmapFromBytes(imageBytes);

            // Save to file for persistence
            saveImageToFile(imageBytes);

            // Redraw if visible
            if (visible) {
                drawFrame();
            }
        }

        private void loadBitmapFromBytes(byte[] imageBytes) {
            try {
                Bitmap newBitmap = BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.length);
                if (newBitmap != null) {
                    synchronized (bitmapLock) {
                        // Recycle old bitmap
                        if (currentBitmap != null && !currentBitmap.isRecycled()) {
                            currentBitmap.recycle();
                        }
                        currentBitmap = newBitmap;
                    }
                    Log.i(TAG, "Loaded bitmap from bytes: " + newBitmap.getWidth() + "x" + newBitmap.getHeight());
                } else {
                    Log.e(TAG, "Failed to decode bitmap from bytes");
                }
            } catch (Exception e) {
                Log.e(TAG, "Error loading bitmap from bytes", e);
            }
        }

        private void loadBitmapFromFile() {
            try {
                File imageFile = new File(getFilesDir(), IMAGE_FILE_NAME);
                if (imageFile.exists()) {
                    Bitmap newBitmap = BitmapFactory.decodeFile(imageFile.getAbsolutePath());
                    if (newBitmap != null) {
                        synchronized (bitmapLock) {
                            if (currentBitmap != null && !currentBitmap.isRecycled()) {
                                currentBitmap.recycle();
                            }
                            currentBitmap = newBitmap;
                        }
                        Log.i(TAG, "Loaded bitmap from file: " + newBitmap.getWidth() + "x" + newBitmap.getHeight());
                    } else {
                        Log.w(TAG, "Failed to decode bitmap from file");
                    }
                } else {
                    Log.w(TAG, "No saved wallpaper image file found");
                }
            } catch (Exception e) {
                Log.e(TAG, "Error loading bitmap from file", e);
            }
        }

        private void saveImageToFile(byte[] imageBytes) {
            try {
                File imageFile = new File(getFilesDir(), IMAGE_FILE_NAME);
                FileOutputStream fos = new FileOutputStream(imageFile);
                fos.write(imageBytes);
                fos.close();
                Log.i(TAG, "Saved image to file: " + imageFile.getAbsolutePath());
            } catch (IOException e) {
                Log.e(TAG, "Error saving image to file", e);
            }
        }

        private void drawFrame() {
            SurfaceHolder holder = getSurfaceHolder();
            Canvas canvas = null;

            try {
                canvas = holder.lockCanvas();
                if (canvas != null) {
                    synchronized (bitmapLock) {
                        if (currentBitmap != null && !currentBitmap.isRecycled()) {
                            drawBitmap(canvas, currentBitmap);
                        } else {
                            // Clear canvas if no bitmap
                            canvas.drawColor(0xFF000000);
                        }
                    }
                }
            } catch (Exception e) {
                Log.e(TAG, "Error drawing frame", e);
            } finally {
                if (canvas != null) {
                    try {
                        holder.unlockCanvasAndPost(canvas);
                    } catch (IllegalArgumentException e) {
                        Log.e(TAG, "Error unlocking canvas", e);
                    }
                }
            }
        }

        private void drawBitmap(Canvas canvas, Bitmap bitmap) {
            int canvasWidth = canvas.getWidth();
            int canvasHeight = canvas.getHeight();
            int bitmapWidth = bitmap.getWidth();
            int bitmapHeight = bitmap.getHeight();

            // Calculate scale to fill canvas (like VIDEO_SCALING_MODE_SCALE_TO_FIT_WITH_CROPPING)
            float scaleX = (float) canvasWidth / bitmapWidth;
            float scaleY = (float) canvasHeight / bitmapHeight;
            float scale = Math.max(scaleX, scaleY);

            // Calculate position to center the bitmap
            float scaledWidth = bitmapWidth * scale;
            float scaledHeight = bitmapHeight * scale;
            float left = (canvasWidth - scaledWidth) / 2;
            float top = (canvasHeight - scaledHeight) / 2;

            // Draw bitmap scaled and centered
            Matrix matrix = new Matrix();
            matrix.postScale(scale, scale);
            matrix.postTranslate(left, top);

            canvas.drawColor(0xFF000000); // Black background
            canvas.drawBitmap(bitmap, matrix, paint);
        }
    }
}
