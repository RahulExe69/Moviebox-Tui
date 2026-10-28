package com.sevenx.cinema;

import android.app.Activity;
import android.app.PictureInPictureParams;
import android.content.res.Configuration;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.util.Rational;
import android.view.View;
import android.view.WindowManager;
import android.widget.FrameLayout;
import android.widget.Toast;

import androidx.media3.common.C;
import androidx.media3.common.MediaItem;
import androidx.media3.common.MediaMetadata;
import androidx.media3.common.MimeTypes;
import androidx.media3.common.PlaybackException;
import androidx.media3.common.Player;
import androidx.media3.datasource.DefaultDataSource;
import androidx.media3.datasource.DefaultHttpDataSource;
import androidx.media3.exoplayer.ExoPlayer;
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory;
import androidx.media3.ui.PlayerView;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

public final class PlayerActivity extends Activity {
    private ExoPlayer player;
    private PlayerView playerView;
    private DefaultHttpDataSource.Factory httpFactory;
    private final List<JSONObject> candidates = new ArrayList<>();
    private String title = "7xCinema";
    private int currentSourceIndex = -1;
    private boolean switchingSource = false;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        enterImmersiveMode();
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        title = getIntent().getStringExtra("media_title");
        if (title == null || title.trim().isEmpty()) title = "7xCinema";
        loadCandidates(
                getIntent().getStringExtra("media_sources"),
                getIntent().getStringExtra("media_url"),
                getIntent().getStringExtra("media_headers"),
                getIntent().getStringExtra("subtitle_url"));

        if (candidates.isEmpty()) {
            Toast.makeText(this, "This source has an invalid playback URL.", Toast.LENGTH_LONG).show();
            finish();
            return;
        }

        playerView = new PlayerView(this);
        playerView.setUseController(true);
        playerView.setControllerShowTimeoutMs(4500);
        playerView.setKeepContentOnPlayerReset(true);
        playerView.setLayoutParams(new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT));
        setContentView(playerView);

        try {
            httpFactory = new DefaultHttpDataSource.Factory()
                    .setAllowCrossProtocolRedirects(true)
                    .setConnectTimeoutMs(15000)
                    .setReadTimeoutMs(30000);
            DefaultDataSource.Factory dataSourceFactory = new DefaultDataSource.Factory(this, httpFactory);
            DefaultMediaSourceFactory mediaSourceFactory = new DefaultMediaSourceFactory(dataSourceFactory);
            player = new ExoPlayer.Builder(this)
                    .setMediaSourceFactory(mediaSourceFactory)
                    .build();
            playerView.setPlayer(player);
            player.addListener(new Player.Listener() {
                @Override
                public void onPlayerError(PlaybackException error) {
                    if (!switchingSource) tryNextSource();
                }
            });
            playSource(0);
        } catch (RuntimeException error) {
            Toast.makeText(this, "Could not start the player for this source.", Toast.LENGTH_LONG).show();
            finish();
        }
    }

    private void loadCandidates(String sourceJson, String fallbackUrl, String fallbackHeaders, String fallbackSubtitle) {
        if (sourceJson != null && !sourceJson.trim().isEmpty()) {
            try {
                JSONArray array = new JSONArray(sourceJson);
                for (int index = 0; index < array.length(); index++) {
                    JSONObject item = array.optJSONObject(index);
                    if (item != null && isHttpUrl(item.optString("url", null))) candidates.add(item);
                }
            } catch (JSONException ignored) {
                // Fall back to the original single-source intent below.
            }
        }

        if (candidates.isEmpty() && isHttpUrl(fallbackUrl)) {
            try {
                JSONObject single = new JSONObject();
                single.put("url", fallbackUrl.trim());
                single.put("headers", new JSONObject(fallbackHeaders == null ? "{}" : fallbackHeaders));
                if (isHttpUrl(fallbackSubtitle)) single.put("subtitleUrl", fallbackSubtitle.trim());
                single.put("label", "Selected source");
                candidates.add(single);
            } catch (JSONException ignored) {
                // Invalid request metadata leaves no valid source candidate.
            }
        }
    }

    private void playSource(int index) {
        if (player == null || httpFactory == null || index < 0 || index >= candidates.size()) return;
        currentSourceIndex = index;
        JSONObject source = candidates.get(index);
        String url = source.optString("url", "");
        String label = source.optString("label", "Source " + (index + 1));
        try {
            Map<String, String> headers = parseHeaders(
                    source.optJSONObject("headers") == null ? "{}" : source.optJSONObject("headers").toString());
            httpFactory.setDefaultRequestProperties(headers);
            String subtitleUrl = source.optString("subtitleUrl", null);

            MediaItem.Builder mediaItem = new MediaItem.Builder()
                    .setUri(Uri.parse(url))
                    .setMediaMetadata(new MediaMetadata.Builder().setTitle(title).build());

            String lowerUrl = url.toLowerCase(java.util.Locale.ROOT);
            if (lowerUrl.contains(".m3u8")) mediaItem.setMimeType(MimeTypes.APPLICATION_M3U8);
            else if (lowerUrl.contains(".mpd")) mediaItem.setMimeType(MimeTypes.APPLICATION_MPD);

            if (isHttpUrl(subtitleUrl)) {
                String subtitleMime = subtitleMimeType(subtitleUrl);
                if (subtitleMime != null) {
                    MediaItem.SubtitleConfiguration subtitle = new MediaItem.SubtitleConfiguration.Builder(
                            Uri.parse(subtitleUrl))
                            .setMimeType(subtitleMime)
                            .setLanguage("und")
                            .setSelectionFlags(C.SELECTION_FLAG_DEFAULT)
                            .build();
                    List<MediaItem.SubtitleConfiguration> subtitles = new ArrayList<>();
                    subtitles.add(subtitle);
                    mediaItem.setSubtitleConfigurations(subtitles);
                }
            }

            switchingSource = true;
            player.stop();
            player.setMediaItem(mediaItem.build());
            player.prepare();
            player.play();
            switchingSource = false;
        } catch (RuntimeException error) {
            switchingSource = false;
            tryNextSource();
        }
    }

    private void tryNextSource() {
        int next = currentSourceIndex + 1;
        if (next < candidates.size()) {
            String label = candidates.get(next).optString("label", "Source " + (next + 1));
            Toast.makeText(this, "Trying another source: " + label, Toast.LENGTH_SHORT).show();
            playSource(next);
        } else {
            Toast.makeText(this, "All available sources failed. Try another provider or title.", Toast.LENGTH_LONG).show();
        }
    }

    private static boolean isHttpUrl(String value) {
        if (value == null || value.trim().isEmpty()) return false;
        Uri uri = Uri.parse(value.trim());
        String scheme = uri.getScheme();
        return uri.getHost() != null && ("http".equalsIgnoreCase(scheme) || "https".equalsIgnoreCase(scheme));
    }

    private static Map<String, String> parseHeaders(String json) {
        java.util.HashMap<String, String> headers = new java.util.HashMap<>();
        if (json == null || json.trim().isEmpty()) return headers;
        try {
            JSONObject object = new JSONObject(json);
            Iterator<String> keys = object.keys();
            while (keys.hasNext()) {
                String key = keys.next();
                Object value = object.opt(key);
                if (value instanceof String && !key.trim().isEmpty()) headers.put(key, (String) value);
            }
        } catch (JSONException ignored) {
            // Invalid optional headers should not prevent trying the URL.
        }
        return headers;
    }

    private static String subtitleMimeType(String rawUrl) {
        String path = Uri.parse(rawUrl).getLastPathSegment();
        if (path == null) return null;
        String lower = path.toLowerCase(java.util.Locale.ROOT);
        if (lower.endsWith(".vtt")) return MimeTypes.TEXT_VTT;
        if (lower.endsWith(".srt")) return MimeTypes.APPLICATION_SUBRIP;
        if (lower.endsWith(".ttml") || lower.endsWith(".xml")) return MimeTypes.APPLICATION_TTML;
        return null;
    }

    /**
     * Collapse playback into Android's draggable Picture-in-Picture window when
     * the user presses Back or leaves the app. Playback stays alive while the
     * underlying 7xCinema catalogue remains usable.
     */
    private boolean enterMiniPlayerMode() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O || player == null
                || player.getPlaybackState() == Player.STATE_IDLE) {
            return false;
        }
        try {
            PictureInPictureParams params = new PictureInPictureParams.Builder()
                    .setAspectRatio(new Rational(16, 9))
                    .build();
            return enterPictureInPictureMode(params);
        } catch (RuntimeException ignored) {
            return false;
        }
    }

    @Override
    public void onBackPressed() {
        if (!isInPictureInPictureMode() && enterMiniPlayerMode()) return;
        super.onBackPressed();
    }

    @Override
    protected void onUserLeaveHint() {
        super.onUserLeaveHint();
        enterMiniPlayerMode();
    }

    @Override
    public void onPictureInPictureModeChanged(boolean inPictureInPictureMode, Configuration newConfig) {
        super.onPictureInPictureModeChanged(inPictureInPictureMode, newConfig);
        if (playerView != null) {
            // Keep the compact window focused on the video; a tap expands it again.
            playerView.setUseController(!inPictureInPictureMode);
        }
        if (inPictureInPictureMode) {
            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        } else {
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            enterImmersiveMode();
        }
    }

    private void enterImmersiveMode() {
        getWindow().getDecorView().setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                        | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                        | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN);
    }

    @Override
    protected void onDestroy() {
        switchingSource = true;
        if (player != null) {
            player.release();
            player = null;
        }
        getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        super.onDestroy();
    }
}
