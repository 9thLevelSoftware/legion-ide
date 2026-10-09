# Windows qualification toolchain inventory

Date: 2026-10-09. Read-only follow-up for ticket 88. These observations identify
installed/default candidates; they do not retroactively identify the compiler,
linker or SDK used for a previous artifact or qualify their product integration.

| Component | Observed identity |
| --- | --- |
| Visual Studio Build Tools 2022 | Display 17.14.37 (July 2026); installation version 17.14.37516.0. |
| Installed root | `C:/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools`. |
| Default MSVC toolset directory | `14.44.35207`, selected by `VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt`. Directory identity differs from patched executable versions below. |
| `Hostx64/x64/cl.exe` | File version 19.44.35228.0; product version 14.44.35228.0; SHA-256 `88c8344236a27a6e727e0a8edc49aaa2690bdc7a9464b9d18cc7abe70a9f1c0d`. |
| `Hostx64/x64/link.exe` | File/product version 14.44.35228.0; SHA-256 `ca11e6c45debd34bf652dfe984c5360a531a005ed78bf72852330c9c2590cf0d`. |
| Windows SDK directory | Installed-root registry value points to `C:/Program Files (x86)/Windows Kits/10/`; `Include` contains `10.0.26100.0`. Package/patch provenance remains separate from this directory identity. |
| rustfmt | `1.9.0-stable (48a229ceae 2026-09-01)`. |
| clippy | `0.1.98 (48a229ceae 2026-09-01)`. |

The compiler/linker paths are relative to
`<installed root>/VC/Tools/MSVC/14.44.35207/bin/`.

## Method and limits

- Queried the installed `vswhere.exe` with `-products '*' -requires
  Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json`.
- Read the default toolset version file, executable `VersionInfo`, and
  `Get-FileHash -Algorithm SHA256` for the two specific x64 executables.
- Read only `KitsRoot10` from
  `HKLM:/SOFTWARE/Microsoft/Windows Kits/Installed Roots` and its Include
  directory names.
- Ran `rustfmt --version` and `cargo clippy --version`.

No tool was installed, upgraded or reconfigured. No credentials, user settings
or unrelated registry data were read. The matching outside-tree observation
note is `D:/legion-ide-2026-notes/windows-toolchain-inventory-2026-10-09.md`.

Future qualification must explicitly bind these paths/hashes, the SDK and source
lock to each build/run record. Default installation metadata alone does not
prove invocation. Other-platform hosts, linkers/SDKs, tool artifacts, fixture
freezes and native/AT sessions remain open in the
[configuration inventory](../completion/ide-2026-language-platform-configurations.md).
