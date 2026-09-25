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

    private static native String nativeProbe();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        String report = nativeProbe()
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
