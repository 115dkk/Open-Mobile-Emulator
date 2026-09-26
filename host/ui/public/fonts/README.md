# Bundled fonts

The webview ships its fonts with the product and never loads a font from the network (R10).
Both families are licensed under the SIL Open Font License 1.1 (`OFL-1.1.txt` in this folder).

| File | Family | Weight | Source | Notes |
|---|---|---|---|---|
| `Pretendard-Regular-subset.woff2` | Pretendard | 400 | https://github.com/orioncactus/pretendard (OFL-1.1) | KS X 1001 Hangul + Latin subset |
| `Pretendard-Medium-subset.woff2` | Pretendard | 500 | same | same |
| `Pretendard-SemiBold-subset.woff2` | Pretendard | 600 | same | same |
| `Pretendard-Bold-subset.woff2` | Pretendard | 700 | same | same |
| `SarasaMonoK-Regular-subset.woff2` | Sarasa Mono K | 400 | https://github.com/be5invis/Sarasa-Gothic (OFL-1.1) | subset |
| `SarasaMonoK-Bold-subset.woff2` | Sarasa Mono K | 700 | same | subset |

The subset files were taken over from the user's EqualizerAPO-XT design system on 2026-09-26
(SHA-256 in the M2 evidence). Before the first release (M3): copy each upstream OFL text with its
copyright line into this folder, confirm whether either family declares a Reserved Font Name (a
subset is a modified version; if a name is reserved, the `@font-face` family in
`ui/src/styles/tokens.css` gets a different name), and list both in `THIRD_PARTY.md`.
