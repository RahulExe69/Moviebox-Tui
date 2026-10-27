package com.sevenx.cinema;

import android.os.Bundle;
import com.getcapacitor.BridgeActivity;

public final class MainActivity extends BridgeActivity {
    @Override
    public void onCreate(Bundle savedInstanceState) {
        registerPlugin(MobileBackendPlugin.class);
        super.onCreate(savedInstanceState);
    }
}
