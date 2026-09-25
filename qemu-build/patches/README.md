# QEMU patches

No patches are supplied yet. An empty patch directory is supported.

Add reviewed patches as `0001-description.patch`, `0002-description.patch`, etc.
The clone step processes `*.patch` in lexical order, checks before applying, and
stages changes in the external QEMU checkout's index. It does not commit to OME.
The source offer archives that index tree, including modified and new files;
archiving HEAD alone would omit applied patches.

Each future patch must identify upstream source, author/license, purpose,
applicable QEMU pin and test evidence. No unofficial Windows/virgl patch is
presumed necessary. The fetched [MSYS2 PKGBUILD][pkgbuild] did not apply an active
Windows patch in its prepare function.

After replacing/removing/reordering patches, inspect local source work before
using `Build-Qemu.ps1 -Clean -IncludeSource`. Do not retain an old patched tree
and claim it matches a changed patch list.

[pkgbuild]: https://raw.githubusercontent.com/msys2/MINGW-packages/refs/heads/master/mingw-w64-qemu/PKGBUILD
