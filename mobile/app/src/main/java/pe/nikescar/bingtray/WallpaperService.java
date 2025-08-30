package pe.nikescar.bingtray;

/**
 * Compatibility shim: some components (or merged manifests) expect
 * a class named pe.nikescar.bingtray.WallpaperService. Provide
 * a lightweight subclass that reuses the existing implementation
 * in BingtrayWallpaperService so the class can be found at runtime.
 */
public class WallpaperService extends BingtrayWallpaperService {
    // No additional code required — inherits behavior from BingtrayWallpaperService
}
