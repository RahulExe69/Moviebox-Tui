package com.sevenx.cinema;

import android.app.Activity;
import android.net.Uri;
import android.os.Bundle;
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

import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

public final class PlayerActivity extends Activity {
    private ExoPlayer player;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        enterImmersiveMode();
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        String url = getIntent().getStringExtra("media_url");
        String title = getIntent().getStringExtra("media_title");
        String headerJson = getIntent().getStringExtra("media_headers");
        String subtitleUrl = getIntent().getStringExtra("subtitle_url");

        if (!isHttpUrl(url)) {
            Toast.makeText(this, "This source has an invalid playback URL.", Toast.LENGTH_LONG).show();
            finish();
            return;
        }

        PlayerView playerView = new PlayerView(this);
        playerView.setUseController(true);
        playerView.setControllerShowTimeoutMs(4500);
        playerView.setKeepContentOnPlayerReset(true);
        playerView.setLayoutParams(new FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT));
        setContentView(playerView);

        try {
            DefaultHttpDataSource.Factory httpFactory = new DefaultHttpDataSource.Factory()
                    .setAllowCrossProtocolRedirects(true)
                    .setConnectTimeoutMs(15000)
                    .setReadTimeoutMs(30000)
                    .setDefaultRequestProperties(parseHeaders(headerJson));
            DefaultDataSource.Factory dataSourceFactory = new DefaultDataSource.Factory(this, httpFactory);
            DefaultMediaSourceFactory mediaSourceFactory = new DefaultMediaSourceFactory(dataSourceFactory);
            player = new ExoPlayer.Builder(this)
                    .setMediaSourceFactory(mediaSourceFactory)
                    .build();
            playerView.setPlayer(player);
            player.addListener(new Player.Listener() {
                @Override
                public void onPlayerError(PlaybackException error) {
                    Toast.makeText(PlayerActivity.this,
                            "Playback failed. Try another source if available.",
                            Toast.LENGTH_LONG).show();
                }
            });

            MediaItem.Builder mediaItem = new MediaItem.Builder()
                    .setUri(Uri.parse(url))
                    .setMediaMetadata(new MediaMetadata.Builder()
                            .setTitle(title == null || title.trim().isEmpty() ? "7xCinema" : title)
                            .build());

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

            player.setMediaItem(mediaItem.build());
            player.prepare();
            player.play();
        } catch (RuntimeException error) {
            Toast.makeText(this, "Could not start the player for this source.", Toast.LENGTH_LONG).show();
            finish();
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
                if (value instanceof String && !key.trim().isEmpty()) {
                    headers.put(key, (String) value);
                }
            }
        } catch (JSONException ignored) {
            // Invalid optional header JSON is ignored; the provider URL still gets a chance.
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
        if (player != null) {
            player.release();
            player = null;
        }
        getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        super.onDestroy();
    }
}
