// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors

package org.ome.arm64probe;

import android.app.Activity;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.widget.TextView;

public final class MainActivity extends Activity {
    private static final String TAG = "OMEProbe";

    static {
        System.loadLibrary("probe");
    }

    private static native String registeredProbe();
    private static native boolean registerDlopenProbe();
    private static native String dlopenProbe();
    private static native String nativeProbe();
    private static native String nativeAssetProbe(android.content.res.AssetManager assets);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        Log.i(TAG, "JNI_OnLoad RegisterNatives CALL");
        String registered = registeredProbe();
        Log.i(TAG, registered);
        Log.i(TAG, "dlopen RegisterNatives BEGIN");
        String loaded = registerDlopenProbe() ? dlopenProbe() : "dlopen registration FAILED";
        Log.i(TAG, loaded);
        Log.i(TAG, "AAssetManager_fromJava BEGIN");
        String assetReport = nativeAssetProbe(getAssets());
        Log.i(TAG, assetReport);
        String report = nativeProbe()
                + "\n" + registered + "\n" + loaded
                + "\n" + assetReport
                + "\nSUPPORTED_ABIS=" + String.join(",", Build.SUPPORTED_ABIS)
                + "\nos.arch=" + System.getProperty("os.arch")
                + "\nCPU_ABI=" + Build.CPU_ABI;
        Log.i(TAG, report.replace('\n', ';'));

        TextView textView = new TextView(this);
        int padding = (int) (24 * getResources().getDisplayMetrics().density);
        textView.setPadding(padding, padding, padding, padding);
        textView.setText(report);
        textView.setTextIsSelectable(true);
        textView.setTextSize(18);
        setContentView(textView);
    }
}
