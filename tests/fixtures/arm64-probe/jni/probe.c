/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) 2026 Open Mobile Emulator contributors */

#include <jni.h>

JNIEXPORT jstring JNICALL
Java_org_ome_arm64probe_MainActivity_nativeProbe(JNIEnv *env, jclass clazz) {
    (void)clazz;
    return (*env)->NewStringUTF(env, "native arm64-v8a lib loaded");
}
