package com.sevenx.cinema;

import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import android.content.Intent;
import android.net.Uri;
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
    public void playVideo(PluginCall call) {
        String url = call.getString("url");
        if (url == null || url.trim().isEmpty()) {
            call.reject("A playback URL is required.");
            return;
        }
        Uri uri = Uri.parse(url.trim());
        String scheme = uri.getScheme();
        if (uri.getHost() == null || !("http".equalsIgnoreCase(scheme) || "https".equalsIgnoreCase(scheme))) {
            call.reject("Only HTTP and HTTPS playback URLs are supported.");
            return;
        }

        String title = call.getString("title", "7xCinema");
        String subtitleUrl = call.getString("subtitleUrl");
        JSObject headers = call.getObject("headers");
        Intent intent = new Intent(getActivity(), PlayerActivity.class);
        intent.putExtra("media_url", url.trim());
        intent.putExtra("media_title", title == null ? "7xCinema" : title);
        intent.putExtra("media_headers", headers == null ? "{}" : headers.toString());
        if (subtitleUrl != null && !subtitleUrl.trim().isEmpty()) {
            Uri subtitle = Uri.parse(subtitleUrl.trim());
            String subtitleScheme = subtitle.getScheme();
            if (subtitle.getHost() == null || !("http".equalsIgnoreCase(subtitleScheme) || "https".equalsIgnoreCase(subtitleScheme))) {
                call.reject("Only HTTP and HTTPS subtitle URLs are supported.");
                return;
            }
            intent.putExtra("subtitle_url", subtitleUrl.trim());
        }

        getActivity().runOnUiThread(() -> {
            try {
                getActivity().startActivity(intent);
                call.resolve();
            } catch (RuntimeException error) {
                call.reject("Could not open the 7xCinema player.");
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
