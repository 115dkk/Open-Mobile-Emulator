/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) 2026 Open Mobile Emulator contributors */
#include <jni.h>

JNIEXPORT jstring JNICALL probe_second(JNIEnv *env, jclass clazz) {
    (void)clazz;
    return (*env)->NewStringUTF(env, "dlopen RegisterNatives PASS");
}
