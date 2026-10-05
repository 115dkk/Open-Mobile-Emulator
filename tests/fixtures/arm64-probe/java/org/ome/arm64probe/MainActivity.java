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

    private static native boolean registerChildProbe(Class<?> child);
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
        String crossLoader;
        try (java.io.InputStream input = getAssets().open("child.dex")) {
            java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[4096];
            int count;
            while ((count = input.read(buffer)) != -1) bytes.write(buffer, 0, count);
            ClassLoader loader = new dalvik.system.InMemoryDexClassLoader(
                    java.nio.ByteBuffer.wrap(bytes.toByteArray()), getClassLoader());
            Class<?> child = loader.loadClass("org.ome.child.RegisteredChild");
            Log.i(TAG, "cross-loader RegisterNatives BEGIN");
            crossLoader = registerChildProbe(child)
                    ? (String) child.getMethod("crossLoaderProbe").invoke(null)
                    : "cross-loader registration FAILED";
        } catch (Exception error) {
            throw new IllegalStateException("cross-loader probe failed", error);
        }
        Log.i(TAG, crossLoader);
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
