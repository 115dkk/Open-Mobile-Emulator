/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) 2026 Open Mobile Emulator contributors */

#include <jni.h>

/* NDK libandroid entry point; keep this probe independent of a full NDK install. */
extern void *AAssetManager_fromJava(JNIEnv *env, jobject asset_manager);

JNIEXPORT jstring JNICALL
Java_org_ome_arm64probe_MainActivity_nativeAssetProbe(JNIEnv *env, jclass clazz,
                                                   jobject asset_manager) {
    (void)clazz;
    void *manager = AAssetManager_fromJava(env, asset_manager);
    return (*env)->NewStringUTF(env, manager ? "AAssetManager_fromJava PASS"
                                           : "AAssetManager_fromJava NULL");
}

JNIEXPORT jstring JNICALL
Java_org_ome_arm64probe_MainActivity_nativeProbe(JNIEnv *env, jclass clazz) {
    (void)clazz;
    return (*env)->NewStringUTF(env, "native arm64-v8a lib loaded");
}
