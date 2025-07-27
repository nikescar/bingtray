package pe.nikescar.bingtray;

import android.app.WallpaperManager;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public class WallpaperSetter {
    private static final String TAG = "WallpaperSetter";
    private Context context;
    private WallpaperSetterCallback callback;
    private ExecutorService executorService;
    private Handler mainHandler;
    
    public interface WallpaperSetterCallback {
        void onPreExecute();
        void onPostExecute(boolean success, String error);
    }
    
    public WallpaperSetter(Context context, WallpaperSetterCallback callback) {
        this.context = context;
        this.callback = callback;
        this.executorService = Executors.newSingleThreadExecutor();
        this.mainHandler = new Handler(Looper.getMainLooper());
    }
    
    public WallpaperSetter(Context context) {
        this.context = context;
        this.callback = null;
        this.executorService = Executors.newSingleThreadExecutor();
        this.mainHandler = new Handler(Looper.getMainLooper());
    }
    
    public void execute(String... paths) {
        // Call onPreExecute on main thread
        mainHandler.post(() -> {
            Log.i(TAG, "Starting wallpaper setting operation...");
            if (callback != null) {
                callback.onPreExecute();
            }
        });
        
        // Execute background task
        executorService.execute(() -> {
            boolean result = doInBackground(paths);
            
            // Call onPostExecute on main thread
            mainHandler.post(() -> {
                String message = result ? "Wallpaper set successfully" : "Failed to set wallpaper";
                Log.i(TAG, "Wallpaper setting completed: " + message);
                
                if (callback != null) {
                    callback.onPostExecute(result, result ? null : "Failed to set wallpaper");
                }
            });
        });
    }
    
    private boolean doInBackground(String... paths) {
        if (paths.length == 0) {
            Log.e(TAG, "No image path provided");
            return false;
        }
        
        String imagePath = paths[0];
        Log.i(TAG, "Setting wallpaper from path: " + imagePath);
        
        try {
            WallpaperManager wallpaperManager = WallpaperManager.getInstance(context);
            File imageFile = new File(imagePath);
            
            if (!imageFile.exists()) {
                Log.e(TAG, "Image file does not exist: " + imagePath);
                return false;
            }
            
            FileInputStream inputStream = new FileInputStream(imageFile);
            wallpaperManager.setStream(inputStream);
            inputStream.close();
            
            Log.i(TAG, "Wallpaper set successfully");
            return true;
            
        } catch (IOException e) {
            Log.e(TAG, "Failed to set wallpaper", e);
            return false;
        } catch (Exception e) {
            Log.e(TAG, "Unexpected error while setting wallpaper", e);
            return false;
        }
    }
    
    public void shutdown() {
        if (executorService != null) {
            executorService.shutdown();
        }
    }
}
