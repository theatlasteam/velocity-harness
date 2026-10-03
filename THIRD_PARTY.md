# Third-party software

- Tauri: MIT/Apache-2.0, https://github.com/tauri-apps/tauri
- React: MIT, https://github.com/facebook/react
- Geist UI and Geist UI Icons: MIT, community implementation, https://github.com/geist-org/geist-ui, https://github.com/geist-org/icons
- Geist font: SIL OFL, https://github.com/vercel/geist-font
- Inter font: SIL OFL, https://github.com/rsms/inter
- React Bits SpotlightCard (also retained BlurText source): MIT + Commons Clause. See REACT-BITS-LICENSE.md. https://github.com/DavidHDev/react-bits
- Rust crates and other JS packages retain their licenses, in Cargo.lock / package-lock.json registries.
- Alpine compatibility variant uses PRoot 5.4.0, GPL-2.0-or-later. Matching upstream source archive is included under third-party-sources. Static binary is from Alpine v3.24 proot-static-5.4.0-r2. Build recipe: https://gitlab.alpinelinux.org/alpine/aports/-/tree/3.24-stable/community/proot
- Static AppImage type2 runtime: https://github.com/AppImage/type2-runtime (MIT).
- Bundled Linux userspace libraries retain their individual distribution licenses, including glibc and WebKitGTK (LGPL families), GTK/GLib and system utilities. They are not relicensed by this project. Matching Amazon Linux source packages and license notices are available from the Amazon Linux 2023 source package repositories / the upstream projects. If redistributing publicly, include required corresponding sources and license notices for your exact build.

No provider credentials are shipped. Provider branding is not used as the app identity.

## Beautiful UI

Source: https://github.com/slev12397/beautiful-ui
Site: https://www.beautifului.dev/
Commit: 44a274e598395ab61e7c96c26fda2758780253b7
MIT copyright (c) 2026 Shane Levine. Full license: BEAUTIFUL-UI-LICENSE.md.
Copied/adapted React primitives: LoadingState, ThinkingState, StreamingText, ToolChips, ApprovalCard, RecommendationCard, CodeBlock, GlideMenu. Atoms: Button, EntityChip, ValuePill. Shared globals.css and lib/utils.ts. Production changes include real-event lifecycle, stable word fade, universal action review, file snapshots/diff, recommendation callbacks and removal of gallery timers from live tool traces. PromptBar and ChatComposer were NOT copied/imported.

## Secure keystore

Pinned `tauri-plugin-secure-keystore` 0.0.3-beta.1, MIT OR Apache-2.0, https://github.com/Abdullah-5603/tauri-plugin-secure-keystore. Used from Rust; no unrestricted frontend secret-reader permission. Android Keystore AES-GCM / desktop OS credentials. Linux requires a user Secret Service; no plaintext fallback.
