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
