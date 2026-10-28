package com.sevenx.cinema;

import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import android.content.Intent;
import android.net.Uri;
import org.json.JSONArray;
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
        if (!isHttpUrl(url)) {
            call.reject("Only HTTP and HTTPS playback URLs are supported.");
            return;
        }

        String title = call.getString("title", "7xCinema");
        String subtitleUrl = call.getString("subtitleUrl");
        JSObject headers = call.getObject("headers");
        JSONArray candidates = new JSONArray();
        JSONArray supplied = call.getData().optJSONArray("sources");

        if (supplied != null) {
            for (int index = 0; index < supplied.length(); index++) {
                JSONObject source = supplied.optJSONObject(index);
                if (source == null) continue;
                String sourceUrl = source.optString("url", "");
                if (!isHttpUrl(sourceUrl)) continue;
                try {
                    JSONObject normalized = new JSONObject();
                    normalized.put("url", sourceUrl.trim());
                    JSONObject sourceHeaders = source.optJSONObject("headers");
                    normalized.put("headers", sourceHeaders == null ? new JSONObject() : sourceHeaders);
                    normalized.put("label", source.optString("label", "Source " + (index + 1)));
                    String sourceSubtitle = source.optString("subtitleUrl", "");
                    if (!sourceSubtitle.isEmpty() && isHttpUrl(sourceSubtitle)) {
                        normalized.put("subtitleUrl", sourceSubtitle.trim());
                    }
                    candidates.put(normalized);
                } catch (Exception ignored) {
                    // Skip an invalid candidate without blocking remaining mirrors.
                }
            }
        }

        if (candidates.length() == 0) {
            JSONObject first = new JSONObject();
            try {
                first.put("url", url.trim());
                first.put("headers", headers == null ? new JSONObject() : new JSONObject(headers.toString()));
                if (subtitleUrl != null && !subtitleUrl.trim().isEmpty()) {
                    if (!isHttpUrl(subtitleUrl)) {
                        call.reject("Only HTTP and HTTPS subtitle URLs are supported.");
                        return;
                    }
                    first.put("subtitleUrl", subtitleUrl.trim());
                }
                first.put("label", "Selected source");
                candidates.put(first);
            } catch (Exception error) {
                call.reject("Could not prepare playback sources.");
                return;
            }
        }

        Intent intent = new Intent(getActivity(), PlayerActivity.class);
        intent.putExtra("media_url", url.trim());
        intent.putExtra("media_title", title == null ? "7xCinema" : title);
        intent.putExtra("media_headers", headers == null ? "{}" : headers.toString());
        intent.putExtra("media_sources", candidates.toString());
        if (subtitleUrl != null && !subtitleUrl.trim().isEmpty() && isHttpUrl(subtitleUrl)) {
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

    private static boolean isHttpUrl(String value) {
        if (value == null || value.trim().isEmpty()) return false;
        Uri uri = Uri.parse(value.trim());
        String scheme = uri.getScheme();
        return uri.getHost() != null && ("http".equalsIgnoreCase(scheme) || "https".equalsIgnoreCase(scheme));
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
