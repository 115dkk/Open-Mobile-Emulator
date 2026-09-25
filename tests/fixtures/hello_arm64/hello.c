/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) 2026 Open Mobile Emulator contributors */

#include <stdio.h>

#if defined(__aarch64__)
#define OME_AARCH64_PRESENT 1
#else
#define OME_AARCH64_PRESENT 0
#endif

int main(void) {
    printf("OME hello from arm64 __aarch64__=%d\n", OME_AARCH64_PRESENT);
    return 0;
}
