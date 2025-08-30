package pe.nikescar.bingtray;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.app.WallpaperManager;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.util.Log;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;

public class BingtrayWallpaperService extends Service {
    
    private static final String TAG = "WallpaperService";
    private static final String CHANNEL_ID = "WallpaperServiceChannel";
    private static final int NOTIFICATION_ID = 1001;
    
    public static final String EXTRA_IMAGE_DATA = "image_data";
    
    private Handler mainHandler;
    private Runnable timeoutRunnable;
    
    @Override
    public void onCreate() {
        super.onCreate();
        Log.i(TAG, "WallpaperService created");
        mainHandler = new Handler(Looper.getMainLooper());
        createNotificationChannel();
    }
    
    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        Log.i(TAG, "WallpaperService started");
        
        // Start foreground service immediately
        Notification notification = createNotification("Setting wallpaper...");
        startForeground(NOTIFICATION_ID, notification);
        
        // Set up safety timeout (30 seconds max)
        timeoutRunnable = () -> {
            Log.w(TAG, "Wallpaper service timeout - stopping service");
            updateNotification("Operation timed out");
            stopSelf();
        };
        mainHandler.postDelayed(timeoutRunnable, 30000);
        
        // Get image data from intent
        final byte[] imageData;
        if (intent != null) {
            imageData = intent.getByteArrayExtra(EXTRA_IMAGE_DATA);
        } else {
            imageData = null;
        }
        
        if (imageData != null && imageData.length > 0) {
            Log.i(TAG, "Received image data: " + imageData.length + " bytes");
            
            // Set wallpaper in background thread
            new Thread(() -> {
                try {
                    setWallpaperFromBytes(imageData);
                    
                    // Update notification to success
                    updateNotification("Wallpaper set successfully!");
                    
                    // Cancel timeout and stop service after longer delay to allow app configuration change
                    mainHandler.removeCallbacks(timeoutRunnable);
                    mainHandler.postDelayed(() -> {
                        Log.i(TAG, "Stopping wallpaper service after successful operation");
                        stopSelf();
                    }, 3000); // Increased delay to 3 seconds to allow main app to handle config changes
                    
                } catch (Exception e) {
                    Log.e(TAG, "Failed to set wallpaper in service", e);
                    updateNotification("Failed to set wallpaper");
                    
                    // Cancel timeout and stop service after showing error on main thread
                    mainHandler.removeCallbacks(timeoutRunnable);
                    mainHandler.postDelayed(() -> {
                        Log.i(TAG, "Stopping wallpaper service after error");
                        stopSelf();
                    }, 2500); // Show error for 2.5 seconds
                }
            }).start();
        } else {
            Log.e(TAG, "No image data provided");
            updateNotification("No image data provided");
            // Cancel timeout and stop service immediately for invalid data, but on main thread
            mainHandler.removeCallbacks(timeoutRunnable);
            mainHandler.post(() -> stopSelf());
        }
        
        return START_NOT_STICKY;
    }
    
    @Override
    public void onDestroy() {
        super.onDestroy();
        
        // Clean up timeout callback if it's still pending
        if (mainHandler != null && timeoutRunnable != null) {
            mainHandler.removeCallbacks(timeoutRunnable);
        }
        
        Log.i(TAG, "WallpaperService destroyed");
    }
    
    @Override
    public IBinder onBind(Intent intent) {
        return null; // Not using binding
    }
    
    private boolean setWallpaperFromBytes(byte[] imageBytes) {
        if (imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "Invalid image data: null or empty");
            return false;
        }

        try {
            Log.i(TAG, "Setting wallpaper from " + imageBytes.length + " bytes in foreground service");
            
            WallpaperManager wallpaperManager = WallpaperManager.getInstance(this);
            
            // Create input stream from bytes
            ByteArrayInputStream inputStream = new ByteArrayInputStream(imageBytes);
            
            // Use setStream method which tends to be more reliable
            wallpaperManager.setStream(inputStream);
            
            inputStream.close();
            
            Log.i(TAG, "Wallpaper set successfully via foreground service");
            return true;
            
        } catch (Exception e) {
            Log.e(TAG, "Failed to set wallpaper via foreground service", e);
            return false;
        }
    }
    
    private void createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            NotificationChannel serviceChannel = new NotificationChannel(
                    CHANNEL_ID,
                    "Wallpaper Service",
                    NotificationManager.IMPORTANCE_LOW
            );
            serviceChannel.setDescription("Handles wallpaper setting operations");
            
            NotificationManager manager = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
            if (manager != null) {
                manager.createNotificationChannel(serviceChannel);
            }
        }
    }
    
    private Notification createNotification(String contentText) {
        Notification.Builder builder;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            builder = new Notification.Builder(this, CHANNEL_ID);
        } else {
            builder = new Notification.Builder(this);
        }
        
        return builder
                .setContentTitle("Bingtray")
                .setContentText(contentText)
                .setSmallIcon(android.R.drawable.ic_menu_gallery) // Using system icon as fallback
                .build();
    }
    
    private void updateNotification(String contentText) {
        Notification notification = createNotification(contentText);
        NotificationManager notificationManager = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
        if (notificationManager != null) {
            notificationManager.notify(NOTIFICATION_ID, notification);
        }
    }
    
    /**
     * Static method to start the wallpaper service from JNI
     */
    public static boolean startWallpaperService(Context context, byte[] imageBytes) {
        if (context == null || imageBytes == null || imageBytes.length == 0) {
            Log.e(TAG, "Invalid parameters for startWallpaperService");
            return false;
        }
        
        try {
            Intent serviceIntent = new Intent(context, BingtrayWallpaperService.class);
            serviceIntent.putExtra(EXTRA_IMAGE_DATA, imageBytes);
            
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(serviceIntent);
            } else {
                context.startService(serviceIntent);
            }
            
            Log.i(TAG, "WallpaperService started with " + imageBytes.length + " bytes");
            return true;
            
        } catch (Exception e) {
            Log.e(TAG, "Failed to start WallpaperService", e);
            return false;
        }
    }
}