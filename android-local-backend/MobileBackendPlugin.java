package com.sevenx.cinema;

import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import org.json.JSONObject;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

@CapacitorPlugin(name = "MobileBackend")
public final class MobileBackendPlugin extends Plugin {
    private static final ExecutorService BACKEND_EXECUTOR = Executors.newSingleThreadExecutor();

    static {
        System.loadLibrary("moviebox_tui");
    }

    private static native String nativeStartServer();
    private static native void nativeStopServer();

    @PluginMethod
    public void start(PluginCall call) {
        BACKEND_EXECUTOR.execute(() -> {
            try {
                JSONObject config = new JSONObject(nativeStartServer());
                if (config.has("error")) {
                    call.reject("Could not start the on-device 7xCinema service: " + config.optString("error"));
                    return;
                }
                JSObject result = new JSObject();
                result.put("baseUrl", config.getString("baseUrl"));
                result.put("token", config.getString("token"));
                call.resolve(result);
            } catch (Exception error) {
                call.reject("Could not start the on-device 7xCinema service.", error);
            } catch (LinkageError error) {
                call.reject("The Rust engine is missing from this APK. Rebuild the Android package.");
            }
        });
    }

    @PluginMethod
    public void stop(PluginCall call) {
        BACKEND_EXECUTOR.execute(() -> {
            try {
                nativeStopServer();
                call.resolve();
            } catch (Exception error) {
                call.reject("Could not stop the on-device 7xCinema service.", error);
            } catch (LinkageError error) {
                call.reject("The Rust engine is unavailable.");
            }
        });
    }
}
