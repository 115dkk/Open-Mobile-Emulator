# SPDX-License-Identifier: GPL-2.0-or-later
# Copyright (C) 2026 Open Mobile Emulator contributors
# Evidence helper used on 2026-09-26 (docs/evidence/M1/whpx-boot.md); needs pycaw (pip install pycaw).
"""Poll the host's audio session peak meters and print one line per second.

Columns: time, endpoint peak, then per-session peak for every session whose
process is qemu-system-x86_64 (or all sessions with --all). Peak values are
WASAPI IAudioMeterInformation::GetPeakValue, 0.0 to 1.0.
"""
import argparse
import sys
import time

from comtypes import CLSCTX_ALL, CoInitialize
from pycaw.pycaw import AudioUtilities, IAudioMeterInformation


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--seconds", type=float, default=10.0)
    parser.add_argument("--interval", type=float, default=0.05)
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--process", default="qemu-system-x86_64.exe")
    args = parser.parse_args()

    CoInitialize()
    device = AudioUtilities.GetSpeakers()
    endpoint_meter = device.EndpointVolume.QueryInterface(IAudioMeterInformation) if hasattr(device, "EndpointVolume") else None
    if endpoint_meter is None:
        endpoint_meter = device._dev.Activate(IAudioMeterInformation._iid_, CLSCTX_ALL, None).QueryInterface(IAudioMeterInformation)

    sessions = AudioUtilities.GetAllSessions()
    watched = []
    for s in sessions:
        name = s.Process.name() if s.Process else "(system)"
        if args.all or name.lower() == args.process.lower():
            meter = s._ctl.QueryInterface(IAudioMeterInformation)
            watched.append((name, s.Process.pid if s.Process else 0, meter))
    print(f"# endpoint: {device.FriendlyName}")
    print(f"# sessions on the endpoint: {[(s.Process.name() if s.Process else '(system)') for s in sessions]}")
    if not watched:
        print(f"# no audio session for {args.process}")
    print("time\tendpoint_peak\t" + "\t".join(f"{n}:{p}" for n, p, _ in watched))
    t_end = time.time() + args.seconds
    while time.time() < t_end:
        t0 = time.time()
        peaks_end = 0.0
        peaks = [0.0] * len(watched)
        while time.time() - t0 < 1.0 and time.time() < t_end:
            peaks_end = max(peaks_end, endpoint_meter.GetPeakValue())
            for i, (_, _, m) in enumerate(watched):
                peaks[i] = max(peaks[i], m.GetPeakValue())
            time.sleep(args.interval)
        print(time.strftime("%H:%M:%S") + f"\t{peaks_end:.3f}\t" + "\t".join(f"{p:.3f}" for p in peaks), flush=True)


if __name__ == "__main__":
    sys.exit(main())
