package pe.nikescar.bingtray;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.os.Handler;
import android.service.wallpaper.WallpaperService;
import android.util.Log;
import android.view.SurfaceHolder;

/**
 * Live wallpaper service for Bingtray
 * Displays static images but follows proper WallpaperService lifecycle
 */
public class BingtrayWallpaperService extends WallpaperService {

    private static final String TAG = "BingtrayWallpaper";
    private static volatile Bitmap sCurrentBitmap = null;
    private static volatile BingtrayEngine sActiveEngine = null;

    @Override
    public Engine onCreateEngine() {
        Log.i(TAG, "Creating wallpaper engine");
        return new BingtrayEngine();
    }

    /**
     * Set wallpaper from image bytes
     * Called from Rust via JNI
     */
    public static boolean setWallpaperFromBytes(byte[] imageBytes) {
        if (imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "setWallpaperFromBytes: empty image data");
            return false;
        }

        try {
            // Decode bitmap from bytes
            Bitmap bitmap = BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.length);
            if (bitmap == null) {
                Log.e(TAG, "Failed to decode bitmap from byte array");
                return false;
            }

            // Store the bitmap
            synchronized (BingtrayWallpaperService.class) {
                // Recycle old bitmap
                if (sCurrentBitmap != null && !sCurrentBitmap.isRecycled()) {
                    sCurrentBitmap.recycle();
                }
                sCurrentBitmap = bitmap;
            }

            Log.i(TAG, "Bitmap updated: " + bitmap.getWidth() + "x" + bitmap.getHeight());

            // Trigger redraw if engine is active
            if (sActiveEngine != null) {
                sActiveEngine.drawFrame();
            }

            return true;
        } catch (Exception e) {
            Log.e(TAG, "setWallpaperFromBytes failed", e);
            return false;
        }
    }

    /**
     * Get current bitmap (for internal use)
     */
    private static Bitmap getCurrentBitmap() {
        synchronized (BingtrayWallpaperService.class) {
            return sCurrentBitmap;
        }
    }

    class BingtrayEngine extends Engine {
        private final Handler mHandler = new Handler();
        private boolean mVisible = false;
        private final Paint mPaint = new Paint();

        @Override
        public void onCreate(SurfaceHolder surfaceHolder) {
            super.onCreate(surfaceHolder);
            Log.i(TAG, "Engine onCreate");

            synchronized (BingtrayWallpaperService.class) {
                sActiveEngine = this;
            }

            mPaint.setAntiAlias(true);
            mPaint.setFilterBitmap(true);
        }

        @Override
        public void onDestroy() {
            super.onDestroy();
            Log.i(TAG, "Engine onDestroy");

            synchronized (BingtrayWallpaperService.class) {
                if (sActiveEngine == this) {
                    sActiveEngine = null;
                }
            }

            mHandler.removeCallbacks(mDrawRunner);
        }

        @Override
        public void onVisibilityChanged(boolean visible) {
            mVisible = visible;
            Log.i(TAG, "Visibility changed: " + visible);

            if (visible) {
                drawFrame();
            } else {
                mHandler.removeCallbacks(mDrawRunner);
            }
        }

        @Override
        public void onSurfaceChanged(SurfaceHolder holder, int format, int width, int height) {
            super.onSurfaceChanged(holder, format, width, height);
            Log.i(TAG, "Surface changed: " + width + "x" + height);
            drawFrame();
        }

        @Override
        public void onSurfaceCreated(SurfaceHolder holder) {
            super.onSurfaceCreated(holder);
            Log.i(TAG, "Surface created");
        }

        @Override
        public void onSurfaceDestroyed(SurfaceHolder holder) {
            super.onSurfaceDestroyed(holder);
            Log.i(TAG, "Surface destroyed");
            mVisible = false;
            mHandler.removeCallbacks(mDrawRunner);
        }

        private final Runnable mDrawRunner = new Runnable() {
            @Override
            public void run() {
                drawFrame();
            }
        };

        void drawFrame() {
            final SurfaceHolder holder = getSurfaceHolder();
            Canvas canvas = null;

            try {
                canvas = holder.lockCanvas();
                if (canvas != null) {
                    drawCanvas(canvas);
                }
            } catch (Exception e) {
                Log.e(TAG, "Error drawing frame", e);
            } finally {
                if (canvas != null) {
                    try {
                        holder.unlockCanvasAndPost(canvas);
                    } catch (Exception e) {
                        Log.e(TAG, "Error unlocking canvas", e);
                    }
                }
            }
        }

        private void drawCanvas(Canvas canvas) {
            Bitmap bitmap = getCurrentBitmap();

            if (bitmap != null && !bitmap.isRecycled()) {
                // Calculate scaling to fill screen while maintaining aspect ratio
                int canvasWidth = canvas.getWidth();
                int canvasHeight = canvas.getHeight();
                int bitmapWidth = bitmap.getWidth();
                int bitmapHeight = bitmap.getHeight();

                float canvasAspect = (float) canvasWidth / canvasHeight;
                float bitmapAspect = (float) bitmapWidth / bitmapHeight;

                Rect srcRect = new Rect(0, 0, bitmapWidth, bitmapHeight);
                Rect dstRect;

                if (canvasAspect > bitmapAspect) {
                    // Canvas is wider - fit to width
                    int scaledHeight = (int) (canvasWidth / bitmapAspect);
                    int offsetY = (canvasHeight - scaledHeight) / 2;
                    dstRect = new Rect(0, offsetY, canvasWidth, offsetY + scaledHeight);
                } else {
                    // Canvas is taller - fit to height
                    int scaledWidth = (int) (canvasHeight * bitmapAspect);
                    int offsetX = (canvasWidth - scaledWidth) / 2;
                    dstRect = new Rect(offsetX, 0, offsetX + scaledWidth, canvasHeight);
                }

                canvas.drawBitmap(bitmap, srcRect, dstRect, mPaint);
            } else {
                // No bitmap - draw black background
                canvas.drawColor(0xFF000000);
            }
        }
    }
}
