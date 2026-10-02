# WIP: §12 prebuilt symdev + rust-sdk — branch `prebuilt`

Brief from the lead (2026-10-02): spec §12 of
`~/worktrees/symdev/toolchain-manager/docs/superpowers/specs/2026-10-02-toolchain-manager-design.md`,
packages side. Worktree `~/worktrees/symdev-packages/prebuilt`. Do not touch `recipes/gcce/`
(branch `gcce-own` changes it). Never push, never merge.

Deliverables: (1) `recipes/symdev/0.1.0/{recipe.toml,build.sh}` → `symdev;0.1.0`
(x86_64-linux, `bin/symdev`, static musl) + `rust-sdk;0.1.0` (any, symbian-rs tree);
recipe format/publish extended for two packages + git source; (2) workflow on
`recipes/symdev/**`; (3) `install.sh` + `tests/install.sh.test`; (4) gates; local musl try.

## Status

| Step | State |
|---|---|
| publish: multi-package recipe, git/tag keys | done 1480ce9 (40+5 tests) |
| recipe + build.sh | done |
| workflow | done |
| install.sh + test | todo |
| local musl attempt | done: fails without musl-gcc (libz-sys), see below |

## Facts

- Receipt (`symdev-sdk/src/receipt.rs`): `toml::to_string` of `{id, sha256, source, url}`,
  file `.symdev-package.toml`, written last via `.symdev-package.toml.partial` + rename;
  `SdkHome::install` stages in `$SYMDEV_HOME/.staging/<pid>-<n>` under `$SYMDEV_HOME/.lock`
  (flock) and **removes all of `.staging`** while holding the lock. `url` = full archive URL.
  Built-in source name `public`, URL `https://pub-15670d2771364287b9982e497c29f586.r2.dev/`.
- `ReproducibleTarGz` modes: dirs and files with any x bit → 0755, else 0644.
- `RustSdk` (symdev-build/src/rust_sdk.rs) reads at build time: `targets/arm-symbian-e32.json`
  (required by `RustSdk::at`), `crates/<name>` (path deps symbian-core/-std → all crates),
  `crates/symbian-libcalls/Cargo.toml` (built with `--manifest-path … --profile libcalls`,
  a profile defined only in the workspace root `Cargo.toml`), `rust-src/overlay{,.toml}`
  (rust-std projects), `shims/common`, `shims/s60` (`*.cpp`, headers beside them).
  `TOOLCHAIN_FILE`/`HELLO_MAIN` are `include_str!` (compiled in, not read at run time).
- symbian-rs: 395 tracked files, all mode 100644, no symlinks; `corpus/` (1.9 MB) holds
  experiment goldens (`.exe`), read by no build.
- symdev workspace (toolchain-manager) uses `ring 0.17.14` via rustls/ureq → C/asm under musl.
- Host: no system gcc; `~/.local/bin/{gcc,cc}` wrap a user-local gcc 15 (`~/.local/native-cc`).

## Decisions

## Next step


## Local musl attempt (2026-10-02)

- `rustup target add x86_64-unknown-linux-musl --toolchain 1.98.1`: ok (user-level rustup).
- Scratch copy: `git archive` of toolchain-manager 553fb0f → `~/src/symdev-musl/src`,
  `CARGO_TARGET_DIR=~/src/symdev-musl/target`.
- `cargo +1.98.1 build --release --locked -p symdev-cli --target x86_64-unknown-linux-musl`
  → fails, log `~/src/symdev-musl/build.log`: `error: failed to run custom build command for
  `libz-sys v1.1.29`` … `error occurred in cc-rs: failed to find tool "x86_64-linux-musl-gcc":
  No such file or directory (os error 2)`. libz-sys comes from symdev-sis's
  `flate2 … features = ["zlib"]`; ring 0.17.14 (rustls ← ureq ← symdev-sdk) needs the same
  compiler (cargo stopped on libz-sys first).
- cc-rs 1.4.6 looks for `x86_64-linux-musl-gcc`, then `musl-gcc` on PATH
  (`find_working_gnu_prefix(&["x86_64-linux-musl", "musl"])`), so Ubuntu's `musl-tools`
  (`/usr/bin/musl-gcc`) is found in CI without a `CC_*` variable.
- Informational only (NOT a release build): `CC_x86_64_unknown_linux_musl=gcc` (host gcc 15,
  glibc headers) → builds in 16 s; `file`: `ELF 64-bit LSB pie executable, x86-64 … static-pie
  linked`, `ldd`: `statically linked`, `readelf -d`: 0 NEEDED, 7 546 896 bytes unstripped;
  `symdev sdk list` with the built-in source reached r2.dev over TLS (HTTP 404: bucket empty).
  So the Rust side of the musl build is fine; only a musl C compiler is missing here.
  build.sh's static check must accept `static-pie linked` as well as `statically linked`.

## rust-sdk file selection (measured 2026-10-02, toolchain-manager 553fb0f)

- `symbian-rs/crates/symbian-macros` depends on `symdev-locale = { path =
  "../../../crates/symdev-locale" }` — a host crate **outside** `symbian-rs/`, which inherits
  `version/edition/license/repository.workspace = true` from the host root `Cargo.toml`. So
  the package keeps the repository layout: package root = repo root, Rust SDK root =
  `<package>/symbian-rs` (symdev side must point `RustSdk` there, not at the package root).
- `cargo metadata --manifest-path <tree>/symbian-rs/crates/symbian-libcalls/Cargo.toml`
  (what the libcalls build loads): symbian-rs alone → `failed to load manifest for
  dependency symdev-locale`; + root Cargo.toml + crates/symdev-locale but no
  `symbian-rs/examples` → `failed to load manifest for workspace member …/examples/async`
  (examples are workspace members); with examples → ok. The host root's other members
  (crates/symdev-core …) are not needed.
- Tree "Dmin" = `Cargo.toml`, `crates/symdev-locale`, `symbian-rs/{Cargo.toml, Cargo.lock,
  rust-toolchain.toml, targets, crates, rust-src, shims, examples}`, made **read-only**
  (`chmod -R a-w`): `symdev new hello --language rust` + `symdev build` (musl symdev from
  the local attempt, classic SYMDEV_* env, `SYMDEV_RUST_SDK=<tree>/symbian-rs`) → ok, so the
  build writes nothing into the package (Cargo.lock present and current). Same project
  against the full checkout at the same path: `hello.elf` identical, `hello.exe` differs
  only at 0x14–0x17 (header CRC) and 0x24–0x27 (time stamp). `examples/ui` (s60 shim) and
  `examples/std-hello` (rust-src overlay, own workspace) also build inside a writable copy
  of Dmin.
- Left out: `symbian-rs/corpus` (experiment goldens), `symbian-rs/.cargo` (symdev passes the
  target and build-std itself), `target/` (never in a git archive). Added: `LICENSE` (MIT
  notice in every copy).

## Recipe + build.sh (2026-10-02)

- `recipes/symdev/0.1.0/recipe.toml`: top `git`/`tag`/`build`, `[[package]]` symdev (no
  include: all of `<out>/symdev`) and rust-sdk (include list = the selection above + LICENSE).
- build.sh against the real recipe: `fatal: Remote branch v0.1.0 not found in upstream origin`
  → `error: cannot clone https://github.com/4akloon/symdev at v0.1.0 (does the tag exist?)`.
- Local run: scratch bare repo (push of toolchain-manager 553fb0f) + scratch tag v0.1.0,
  recipe copy with `git = file://…`, `CC_x86_64_unknown_linux_musl=gcc` (informational) →
  ok; static-pie binary 7 546 912 bytes, source archive 3 018 564 bytes. Guards checked:
  ids 0.2.0 vs workspace 0.1.0; missing rust-sdk id; a branch named like the tag ("is not a
  tag"); static check fails on a dynamic binary (both the `file -bL` and the `ldd` branch).
- `publish public … --dry-run` of both from build.sh's out: rust-sdk 377 613 bytes
  (325 files), symdev 3 029 988 bytes; both index entries right. Extracted read-only into a
  fake SYMDEV_HOME: the packed static symdev + `SYMDEV_RUST_SDK=<rust-sdk>/symbian-rs`
  scaffold and build a Rust hello.
- No shellcheck on this host (dash and busybox are).

## Workflow (2026-10-02)

- New `.github/workflows/symdev.yml` (build.yml/publish.yml untouched except build.yml's PR
  filter `recipes/**` → `recipes/gcce/**`, so a symdev recipe PR does not start a GCCE build).
- Jobs: `build` (ubuntu-24.04 runner — a static binary needs no old glibc; apt musl-tools;
  rustup 1.98.1 + musl target; choose the recipe dir the change touches (PR base / push
  before; none → newest; >1 → error; dispatch input validated); build.sh; dry-run both
  packages against `vars.PUBLIC_READ_URL`; tar of out/ as artifact), `publish-rust-sdk`, then
  `publish-symdev` (each `environment: publish`, `concurrency: publish`, push/dispatch only).
  Two jobs so "Re-run failed jobs" after a symdev failure does not stop at rust-sdk's
  "already published".
- Checked: YAML parses (python yaml); the choose step run in a mock repo for 7 cases
  (publisher-only → newest, new 0.2.0, two versions → error, dispatch ok/`../x`/missing,
  zero `before`). Not runnable until R4 (publish's path dep), noted in the header.

---

# WIP: Track E (publisher) — branch `publish`

Plan: `/home/genius/worktrees/symdev/toolchain-manager/docs/superpowers/plans/2026-10-02-toolchain-manager.md`
Track E (E1–E3). Spec §2, §6 of `…/specs/2026-10-02-toolchain-manager-design.md`.
Worktree `~/worktrees/symdev-packages/publish`. Do not touch `recipes/gcce/` or `main`
(Track C commits there). Never push, never add a remote, never merge.

## Status

| Task | State |
|---|---|
| E1 `publish` binary | done (35 tests) |
| E2 SDK recipe + subset proof | done |
| E3 workflows | written, blocked: no Debian 11 apt list yet (experiment 107) |

## Facts

- `symdev-sdk` (path dep on `~/worktrees/symdev/toolchain-manager/crates/symdev-sdk`):
  `HttpFetch::put_file(url, file, sha256, content_type, cache_control)`; a missing object is
  `SdkError::Fetch { detail: "HTTP 404" }`; `ReproducibleTarGz::pack(root, include, out)`
  accepts `"."`; `Index::insert` refuses an existing id; `SigV4::s3(keys, "auto")`;
  `SourceSpec::new(name, url, auth)` validates a base URL (https, or http to localhost).
- The plan's include `epoc32/release/armv5/lib/usrt2_2.lib` does not exist in
  `~/sdk/S60_3rd_FP2`: `find -iname 'usrt2_2*'` → `epoc32/release/armv5/{urel,udeb}/usrt2_2.lib`
  only. symdev's link line (`symdev-build/src/driver/link.rs`) passes
  `-L <epocroot>/epoc32/release/armv5/urel` and `-l:usrt2_2.lib`, so the recipe takes
  `epoc32/release/armv5/urel/usrt2_2.lib`.
- No symlinks under `epoc32/include`, `epoc32/release/armv5`, `epoc32/tools/variant`; 570
  `*.dso` in `epoc32/release/armv5/lib`.
- GCCE recipe (Track C, read-only, `recipes/gcce/12.1.0/`): `recipe.toml` has `id`,
  `license`, `host`, `build = "build.sh"` and `[[source]]` tables (`url`, `sha256`).

## Decisions

- E1 layout: `Recipe` (recipe.rs, `deny_unknown_fields`; `build`/`[[source]]` accepted and
  ignored), `Include` (one include pattern, `*` only in the last segment, matches any run of
  characters incl. a leading dot; a symlink in the tree is an error — never observed in the
  SDK), `Archive` (path + sha + size), `Bucket` (S3 endpoint over `HttpFetch`),
  `Publication` (the flow), `Mode` (`DryRun(Option<Bucket>)` / `Upload(Bucket)`),
  `Visibility`, `Settings` (the only env reader).
- The archive is written to the current directory as `<sha256>.tar.gz` (via a `.part` file)
  and kept: a dry run leaves it for inspection / the CI artifact. Staging for an include
  list is a system temp dir, removed afterwards; packing uses `pack(staging, ["."])`.
- Keys: archive `<id path>/<sha>.tar.gz`, source `src/<id path>/<sha>.tar.gz`, index
  `index.toml` at the bucket root.
- Index read: only `HTTP 404` means "no index yet"; any other failure stops (test with a
  bucket answering 500; mutation `starts_with("HTTP")` makes it fail).
- Env checked before packing: an upload needs the bucket URL and both key halves; a dry run
  needs nothing (no URL → empty index, URL without keys → unsigned GET, e.g. r2.dev);
  half a key is an error in both modes.
- Additions beyond the plan (cheap guards): the `<id>` argument must equal the recipe's id;
  `public` refuses a `LicenseRef-…` licence; `public` needs `--source-code`; a `sha256` in a
  public recipe is checked too; a private recipe without `sha256` stops with the exact
  line to record.
- `cargo fmt` in this repo does not touch the symdev-sdk path dep (checked: toolchain-manager
  worktree stayed clean).

## E2 results (2026-10-02)

- Plan's literal include list: `error: include `epoc32/release/armv5/lib/usrt2_2.lib`
  matches nothing in /home/genius/sdk/S60_3rd_FP2` (exit 1). Recipe uses
  `epoc32/release/armv5/urel/usrt2_2.lib` instead (the spec §2 text "usrt2_2.lib there
  [lib]" has the same error — for the lead to fix in the spec/plan).
- Two dry runs (scratch `…/scratchpad/e2/run1`, `run2`, no `PUBLISH_*` set):
  sha256 `cbec6da885cfa11bfc5898b544c30fa961b57e57c52641b12d463380ca162a5f`, 4 941 155
  bytes, both times; ~1 s each. 2 697 files: 2 123 headers, 570 `.dso`, 3 `.lib`,
  `variant.cfg`; 31 MB unpacked. `diff -r` of `epoc32/include` against the SDK: identical;
  the other 574 files `cmp`-equal. Pinned in the recipe; a third dry run passes and
  prints the index.
- Extracted with `symdev_sdk::TarGz::extract` (scratch crate) → same tree as system tar.
- Build proof (scratch `e2/build.sh`: fresh copy of the toolchain-manager tree at one fixed
  path, `LD_PRELOAD` clock shim built in scratch from Track C's `fixtime.c` source so the
  E32 header time is fixed, env as in the brief, symdev from toolchain-manager
  `target/release`): `examples/hello`, `examples/gui`, `symbian-rs/examples/hello`, and
  `calc` + `mathlib.dll` all build with `SYMDEV_EPOCROOT=<subset>` and with
  `~/sdk/S60_3rd_FP2`. `cmp`: every `.exe`, `.dll`, `.elf`, `.o`, `.dso`, `.def`, `.rsc`,
  `.rsg`, `.mif`, `.mbg`, `symdev-gcce-compat.h` identical. Different only:
  `*.map` (identical after replacing the EPOCROOT prefix), `sdk-include-casefold/.symdev-casefold`
  (holds the EPOCROOT include path), Rust `shims/libsymrs.a` (7 members equal in name,
  mode, size and bytes; only ar member mtimes differ — the `.o` files' real mtimes).
- No DLL project exists in the repo (examples, `crates/symdev-cli/tests`, fixtures: only
  `Mmp::parse` unit strings). Reconstructed in scratch from experiment 53's recorded
  procedure: `symdev new calc`, `group/mathlib.mmp` (`TARGETTYPE DLL`,
  `UID 0x1000008d 0xe5d1b001`) before `calc.mmp`, `LIBRARY mathlib.lib`, experiment 52's
  `mathlib.{h,cpp}` from `~/src/symdev-experiment-52`, `hello.cpp` printing
  `MathTwice(21)`/`MathAbs(-5)`.

## E3 (2026-10-02)

- Track C has not recorded the Debian 11 apt list: `main` of symdev-packages is still at
  ca00929, tm-gcce's note ends "Next step: … experiment 107; Debian 11 apt list", and
  experiment 107 is not in tm-gcce's backlog. So `build.yml`'s install step is a
  `TODO … (not observed)` that exits 1; nothing guessed.
- Design: job `gcce` in `container: debian:11` runs only build.sh (+ `g++ -v`, the
  corresponding-source tarball from build.sh's SHA256SUMS + recipe + build.sh, and a tar of
  `/opt/gcce` because artifacts lose modes and symlinks); job `pack` (build.yml) /
  `publish` (publish.yml) runs `publish` on `ubuntu-24.04`, so the container needs no Rust
  and no apt packages beyond experiment 107's. publish.yml calls build.yml
  (`workflow_call`), then its `publish` job has `environment: publish` and
  `concurrency: publish` (job level: only the index read-modify-write is serialised).
- Configuration the owner sets: repo variable `PUBLIC_READ_URL` (r2.dev, for PR dry runs;
  unset → empty index); environment `publish`: variable `PUBLISH_PUBLIC_URL` (S3 endpoint),
  secrets `PUBLISH_ACCESS_KEY_ID`, `PUBLISH_SECRET_ACCESS_KEY`.
- Action versions mirror symdev's ci.yml on toolchain-manager (checkout@v7,
  upload-artifact@v5; download-artifact@v5 to match).
- Checked locally: YAML parses (`python3 yaml.safe_load`; no actionlint on this host); the
  source-pack shell snippet run on a mock work dir gives a correct tarball; `publish public
  'gcce;12.1.0' --dry-run` on a scratch copy of Track C's relocated prefix
  (`~/src/gcce-recipe/moved`, 201 MiB, 6 symlinks, 24 hard-linked files) → 72 365 105 bytes
  in 8.5 s; `TarGz::extract` of it equals the prefix (`diff -r --no-dereference`), g++ runs.
- Also blocked until R4: `cargo run --locked -p publish` needs symdev-sdk as a git dep.

## Next step

E3: when Track C records the Debian 11 apt list (experiment 107), replace the TODO step in
`.github/workflows/build.yml` with `apt-get update && apt-get install -y --no-install-recommends <list>`.

E3: workflows; needs Track C's apt list (experiment 107) — check `recipes/gcce/12.1.0/` and the tm-gcce wip note.
