/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) 2026 Open Mobile Emulator contributors */

#include <jni.h>

extern void *dlopen(const char *filename, int flags);
extern void *dlsym(void *handle, const char *symbol);

static jstring registered_probe(JNIEnv *env, jclass clazz) {
    (void)clazz;
    return (*env)->NewStringUTF(env, "JNI_OnLoad RegisterNatives PASS");
}

JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM *vm, void *reserved) {
    (void)reserved;
    JNIEnv *env = 0;
    if ((*vm)->GetEnv(vm, (void **)&env, JNI_VERSION_1_6) != JNI_OK) return JNI_ERR;
    jclass clazz = (*env)->FindClass(env, "org/ome/arm64probe/MainActivity");
    if (!clazz) return JNI_ERR;
    JNINativeMethod method = {"registeredProbe", "()Ljava/lang/String;", (void *)registered_probe};
    return (*env)->RegisterNatives(env, clazz, &method, 1) == JNI_OK ? JNI_VERSION_1_6 : JNI_ERR;
}

JNIEXPORT jboolean JNICALL
Java_org_ome_arm64probe_MainActivity_registerDlopenProbe(JNIEnv *env, jclass clazz) {
    /* RTLD_NOW=2 on bionic. Keep the handle alive for the registered method. */
    void *handle = dlopen("libprobe_second.so", 2);
    if (!handle) return JNI_FALSE;
    void *function = dlsym(handle, "probe_second");
    if (!function) return JNI_FALSE;
    JNINativeMethod method = {"dlopenProbe", "()Ljava/lang/String;", function};
    return (*env)->RegisterNatives(env, clazz, &method, 1) == JNI_OK;
}

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
