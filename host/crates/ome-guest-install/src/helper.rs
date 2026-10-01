// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Open Mobile Emulator contributors
//! Install-helper embedding, initrd construction and kernel command lines.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use crate::cpio::{self, Entry};

/// The install helper sourced by the Bliss initrd, embedded verbatim.
pub const HELPER_SCRIPT: &str = include_str!("../scripts/99-ome-install");
/// Where the helper lives inside the initrd (the initrd's /init sources every file under /scripts).
pub const HELPER_PATH: &str = "scripts/99-ome-install";
/// Default folder on the new partition that holds kernel, initrd.img, system.img and data/.
pub const DEFAULT_SRC: &str = "ome";

/// Writes the bytes of `initrd` followed by a newc archive holding the directory "scripts"
/// (mode 0o040755) and the file "scripts/99-ome-install" (mode 0o100644) to `destination`.
/// CRLF sequences in the embedded script are normalised to LF before writing.
pub fn write_install_initrd(initrd: &Path, destination: &Path) -> io::Result<()> {
    let mut source = File::open(initrd)?;
    let mut output = File::create(destination)?;
    io::copy(&mut source, &mut output)?;

    let helper = HELPER_SCRIPT.replace("\r\n", "\n");
    let archive = cpio::newc_archive(&[
        Entry::Directory {
            name: "scripts",
            mode: 0o040755,
        },
        Entry::File {
            name: HELPER_PATH,
            mode: 0o100644,
            data: helper.as_bytes(),
        },
    ]);
    output.write_all(&archive)
}

/// Kernel command line of the helper boot:
/// "root=/dev/ram0 console=ttyS0 OME_INSTALL=1 OME_DISK=<disk> OME_SRC=<src>".
#[must_use]
pub fn install_cmdline(disk: &str, src: &str) -> String {
    format!("root=/dev/ram0 console=ttyS0 OME_INSTALL=1 OME_DISK={disk} OME_SRC={src}")
}

/// Kernel command line of the installed guest: "root=/dev/ram0 SRC=/<src>" followed by each
/// extra argument, all separated by single spaces (no trailing space; empty `extra` gives no trailer).
#[must_use]
pub fn boot_cmdline(src: &str, extra: &[String]) -> String {
    let base = format!("root=/dev/ram0 SRC=/{src}");
    if extra.is_empty() {
        base
    } else {
        format!("{base} {}", extra.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{HELPER_PATH, HELPER_SCRIPT, boot_cmdline, install_cmdline, write_install_initrd};

    static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

    fn temp_path(label: &str) -> PathBuf {
        let sequence = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ome-guest-install-{label}-{}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn appends_helper_archive_to_original_initrd() {
        let source = temp_path("source-initrd");
        let destination = temp_path("install-initrd");
        let original = b"known initrd bytes";
        fs::write(&source, original).expect("write source initrd");

        write_install_initrd(&source, &destination).expect("write install initrd");
        let output = fs::read(&destination).expect("read install initrd");

        assert!(output.starts_with(original));
        let archive = &output[original.len()..];
        assert!(archive.starts_with(b"070701"));
        assert!(
            archive
                .windows(HELPER_PATH.len())
                .any(|window| window == HELPER_PATH.as_bytes())
        );
        assert!(
            archive
                .windows(HELPER_SCRIPT.len())
                .any(|window| window == HELPER_SCRIPT.as_bytes())
        );

        fs::remove_file(source).expect("remove source initrd");
        fs::remove_file(destination).expect("remove install initrd");
    }

    #[test]
    fn embeds_lf_only_spdx_script() {
        assert!(!HELPER_SCRIPT.contains('\r'));
        assert!(HELPER_SCRIPT.starts_with("# SPDX-License-Identifier: GPL-2.0-or-later\n"));
    }

    #[test]
    fn composes_exact_kernel_command_lines() {
        assert_eq!(
            install_cmdline("/dev/vda", "ome"),
            "root=/dev/ram0 console=ttyS0 OME_INSTALL=1 OME_DISK=/dev/vda OME_SRC=ome"
        );
        assert_eq!(boot_cmdline("ome", &[]), "root=/dev/ram0 SRC=/ome");
        assert_eq!(
            boot_cmdline(
                "custom",
                &["quiet".to_owned(), "video=1920x1080".to_owned()]
            ),
            "root=/dev/ram0 SRC=/custom quiet video=1920x1080"
        );
    }
}
