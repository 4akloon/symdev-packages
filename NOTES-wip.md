# WIP: Track E (publisher) — branch `publish`

Plan: `/home/genius/worktrees/symdev/toolchain-manager/docs/superpowers/plans/2026-10-02-toolchain-manager.md`
Track E (E1–E3). Spec §2, §6 of `…/specs/2026-10-02-toolchain-manager-design.md`.
Worktree `~/worktrees/symdev-packages/publish`. Do not touch `recipes/gcce/` or `main`
(Track C commits there). Never push, never add a remote, never merge.

## Status

| Task | State |
|---|---|
| E1 `publish` binary | done (35 tests) |
| E2 SDK recipe + subset proof | not started |
| E3 workflows | not started |

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

## Next step

E2: SDK recipe, two dry runs in a scratch dir, subset build proof.
