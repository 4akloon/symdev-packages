# symdev-packages

Recipes and the publisher for the packages [symdev](https://github.com/4akloon/symdev)
installs by itself (`symdev sdk install`, or the first `symdev build`). Design: symdev's
`docs/superpowers/specs/2026-10-02-toolchain-manager-design.md`, §2 and §6.

This repository never holds a proprietary file: no SDK, firmware, `.sis`, `.sisx`, `.cer`
or `.key` bytes. The SDK recipe lists paths and pins a hash; the bytes stay on the owner's
machine and in the private bucket. CI installs the SDK on its runner only to compile the
Rust SDK's prebuilt shims, and checks that no file of it reaches an artifact.

## Layout

```
recipes/gcce/12.1.0/recipe.toml          # sources + sha256; built by build.sh in CI
recipes/gcce/12.1.0/build.sh
recipes/sdk/s60-3rd-fp2/1.1/recipe.toml  # paths taken from the SDK, pinned archive sha256
recipes/symdev/<ver>/recipe.toml         # symdev;<ver> + rust-sdk;<ver> from the tag v<ver>
recipes/symdev/<ver>/build.sh            #   (0.1.0, 0.2.0 published; 0.3.0 waits for its tag)
recipes/symdev/0.3.0/prebuilt.sh         # rust-sdk's prebuilt/ (from 0.3.0)
install.sh                               # installs the newest symdev from the public bucket
publish/                                 # the publisher (Rust, on symdev-sdk)
pkgtools/                                # the pipeline's build checks (Rust), see pkgtools
tools/notices/gcc-12.1.0/                # COPYING3, COPYING.RUNTIME shipped in prebuilt/
tools/notices/musl-1.2.5/                # musl's COPYRIGHT, compiled into pkgtools notices
tests/                                   # install.sh.test
.github/workflows/                       # build.yml (PRs, no upload), publish.yml (main),
                                         # symdev.yml, tests.yml
```

## Recipes

| Key | Meaning |
|---|---|
| `id` | package id, e.g. `gcce;12.1.0`; never reused for different bytes |
| `license` | SPDX expression; a `LicenseRef-…` package is never published publicly |
| `host` | `x86_64-linux` or `any` |
| `include` | optional: paths relative to `--from` (a `*` only in the last segment); without it all of `--from` is packed |
| `sha256` | the archive's hash; required for `private`, checked whenever present |
| `build`, `[[source]]` | the build script and its pinned sources (read by `build.sh` and CI) |
| `git`, `tag` | a source tree instead of tarballs: the repository and the tag `build.sh` checks out |
| `commit` | with `git` and `tag`: the 40 lowercase hex digits of the commit the tag must be; `build.sh` fails unless the clone is that commit (from 0.3.0; all zeros = not set yet, which `build.sh` refuses) |

A build that makes several packages (the symdev release makes `symdev` and `rust-sdk` from
one tag) keeps `git`, `tag`, `build` and `[[source]]` at the top and gives each package a
`[[package]]` table with its own `id`, `license`, `host`, `include` and `sha256`. `publish`
takes the package named by its `<id>` argument and checks all of them.

## The prebuilt set (rust-sdk from 0.3.0)

From `rust-sdk;0.3.0` the Rust SDK carries `symbian-rs/prebuilt/`: symdev's C++ shims
compiled once for every Rust application, and the four GCC runtime members they need, so
that symdev can link a Rust application with rust-lld and no GCCE on the machine (symdev
experiment 109). About 90 KB:

| File | What | Licence |
|---|---|---|
| `lib/libsymrs.a` | `symbian-rs/shims/common/*.cpp`, linked into every Rust application | MIT |
| `lib/libsymrs_ui.a` | `symbian-rs/shims/s60/*.cpp`, for an `[ui]` (Avkon) application | MIT |
| `lib/libsupc++.a` | `del_ops.o`, `eh_personality.o` of GCC 12.1.0's libsupc++ | GPL-3.0-or-later WITH GCC-exception-3.1 |
| `lib/libgcc.a` | `pr-support.o`, `_thumb1_case_uqi.o` of GCC 12.1.0's libgcc | GPL-3.0-or-later WITH GCC-exception-3.1 |
| `NOTICE`, `COPYING3`, `COPYING.RUNTIME` | what each file is, its licence, where its source is | |

Hence the package's licence, `MIT AND GPL-3.0-or-later WITH GCC-exception-3.1`. The runtime
members are GCC's own, only stripped of debug information; their Corresponding Source is the
source archive of `gcce;12.1.0` in the same bucket (the GCC 12.1.0 tarball and the recipe
that built GCCE), which `NOTICE` names. The shims are compiled against the S60 SDK's
headers, so their objects hold what those headers inline, but no file of the SDK.
`symrs_avkon.o` is compiled without `SYMRS_UID3`: it reads the application's UID3 from the
symbol `symrs_uid3`, which the application's rust-lld link defines
(`--defsym=symrs_uid3=0x<uid3>`); symdev's own GNU ld link cannot, so it keeps compiling that
shim per application with the define.

`recipes/symdev/<ver>/prebuilt.sh <symdev-src> <gcce> <epocroot> <out-dir>` makes it after
`build.sh`, from the tag's tree, the installed `gcce;12.1.0` and the installed
`sdk;s60-3rd-fp2;1.1`:

1. it compiles each shim with the argv of symdev's own GNU build (GcceBuild's recorded line,
   `RustBuild::SHIM_OPTIONS`, which it checks against the tag, and the SDK's case-fold
   overlay, `pkgtools sdk-casefold`), less `-DSYMRS_UID3`; the objects are byte-identical to
   the ones symdev compiles per application;
2. rewrites their `R_ARM_TARGET2` relocations to `R_ARM_ABS32` (`pkgtools target2-abs32`);
3. takes the four members out of GCCE's `libsupc++.a` and `libgcc.a` and strips their debug
   information;
4. links every shim with GCCE's `ld` against the SDK's import libraries and GCCE's full
   runtime libraries, and fails unless that link took exactly the four members
   (`pkgtools runtime-closure`) and the same link against `lib/` alone leaves the same
   symbols undefined. A shim that starts to need another runtime member fails here.

The archives are written with `ar crD`, so the same inputs give the same bytes. A compile
error shows GCC's diagnostics without the source lines it quotes, which may be the SDK's.
`symdev.yml` runs it (see CI); by hand, in the directory where `build.sh` ran (it holds the
tag's tree as `./symdev`), with this repository at `$PACKAGES`:

```bash
symdev sdk install 'gcce;12.1.0' 'sdk;s60-3rd-fp2;1.1'   # the SDK needs the private source
cargo build --release --locked --manifest-path "$PACKAGES/Cargo.toml" -p pkgtools
export PKGTOOLS=$PACKAGES/target/release/pkgtools          # build.sh needs it too
home=${SYMDEV_HOME:-~/.local/share/symdev}
bash "$PACKAGES/recipes/symdev/0.3.0/prebuilt.sh" symdev "$home/gcce/12.1.0" \
  "$home/sdk/s60-3rd-fp2/1.1" <build.sh's out-dir>/rust-sdk/symbian-rs/prebuilt
```

The recipe's `include` names `symbian-rs/prebuilt`, so `publish` does not pack a rust-sdk
without it (an entry that matches nothing is refused), and `build.sh` refuses a tag that
tracks `symbian-rs/prebuilt` itself.

## Publishing

```
publish private <id> --from <dir> --recipe <recipe.toml> [--dry-run]
publish public  <id> --from <prefix> --source-code <tar.gz> --recipe <recipe.toml> [--dry-run]
publish file <path> --to <key> --bucket public|private --content-type <type> \
             --cache-control <value> [--dry-run]
publish sign-index --bucket public|private [--accept-unsigned <sha256>] [--dry-run]
```

`publish` packs the package reproducibly into `<sha256>.tar.gz` in the current directory,
checks the hash against the recipe, reads the bucket's `index.toml` (none yet = empty),
refuses an id that is already there, then uploads the archive (and for `public` the source
archive) and the index last. `--dry-run` uploads nothing and prints the index it would
write.

Every index is signed (symdev spec §14): its first line is `# symdev-signature: ed25519
<base64>`, the project key's Ed25519 signature of the rest, which symdev and `install.sh`
check with the public key they carry. `publish` signs the index it writes with
`PUBLISH_SIGNING_KEY`: an upload needs it, and it must be a key symdev trusts (a stale or
mistyped one would sign an index every client refuses); a dry run signs when it is set and
says when it is not, or when symdev would not trust it. It extends only an index whose
signature verifies: one that does not is refused, and an unsigned one is refused by an
upload (a dry run warns), since otherwise the next publish would sign whatever someone
holding the bucket's R2 key wrote. `publish sign-index` signs the bucket's current index as
it is, byte for byte, after checking that any signature it has verifies and that it parses,
and lists its archives first. It is the one way to accept an unsigned index (the indexes
published before 0.2.0), and only for the exact bytes reviewed: its `--dry-run` prints the
index's SHA-256, and the real run signs it only with `--accept-unsigned <that sha256>`.

`publish file` puts one file as it is at a fixed key (`install.sh` at the public bucket's
root), signed like the other uploads, with the two headers given; it neither reads nor
writes the index. The object is mutable: the next upload replaces it. A key may only hold
`A-Z a-z 0-9 - . _ ~` and `/`, and may not start with `/`, have an empty, `.` or `..`
segment, or be `index.toml`. `--dry-run` hashes the file and says what it would upload.

| Variable | For |
|---|---|
| `PUBLISH_PRIVATE_URL`, `PUBLISH_PUBLIC_URL` | the buckets' S3 endpoints, e.g. `https://<account>.r2.cloudflarestorage.com/symdev-private/` |
| `PUBLISH_ACCESS_KEY_ID`, `PUBLISH_SECRET_ACCESS_KEY` | the publisher key (R2 object read & write) |
| `PUBLISH_SIGNING_KEY` | the index signing key: the base64 of the project key's 32-byte Ed25519 seed (the owner's `~/.config/symdev/keys.env`) |

A dry run needs none of them; with the bucket URL set it also reads the current index, and
with the signing key it signs the index it prints. `publish file` never needs the signing
key.

The SDK is published by the owner from their own copy:

```bash
cargo run --release -p publish -- private 'sdk;s60-3rd-fp2;1.1' \
  --from ~/sdk/S60_3rd_FP2 --recipe recipes/sdk/s60-3rd-fp2/1.1/recipe.toml
```

## pkgtools

Every tool the pipeline runs besides the publisher, in Rust (`cargo build --release --locked
-p pkgtools`); the recipes' scripts take it from `PKGTOOLS`. Offline: no bucket, no key.
Exit 2 is a usage error.

```
pkgtools notices --manifest-path <checkout>/Cargo.toml --package symdev-cli \
                 --target x86_64-unknown-linux-musl --output <file>
pkgtools sdk-casefold <epoc32/include> <out-dir>
pkgtools target2-abs32 <object.o>...
pkgtools runtime-closure <ld.map> <archive>(<member>)...
pkgtools sdk-free <sdk-dir> <path>...
```

| Subcommand | Used by | What |
|---|---|---|
| `notices` | build.sh | THIRD-PARTY-NOTICES.txt of the static symdev: every crate `cargo metadata --filter-platform <target>` resolves from the package through normal dependencies (not the workspace's own), the full text of its licence files, the code it bundles (a table; a crate with `links` or a nested licence file the table does not name is an error), and the toolchain's runtime (std from `COPYRIGHT-library.html`, musl's COPYRIGHT from `tools/notices/`, LLVM's entry of `COPYRIGHT.html`), checked against the runtime archives; needs cargo and rustc. Exit 1 on any error, and no file. |
| `sdk-casefold` | prebuilt.sh | symdev's case-insensitive overlay of the SDK headers: a symlink for every `#include` name the tree has only in another case (the first in sorted order on a tie); prints the overlay; reuses one marked as built |
| `target2-abs32` | prebuilt.sh | `R_ARM_TARGET2` → `R_ARM_ABS32` in place, with symdev's own `Target2Rewrite`; prints the count per file; exit 1 for anything but an ELF32 little-endian ARM `ET_REL` |
| `runtime-closure` | prebuilt.sh | the GCC runtime members a GNU ld `-Map` took from the named archives must be exactly the shipped ones; prints each and why; exit 1 naming member, referrer and symbol |
| `sdk-free` | symdev.yml | no file under the paths (directories, tar, .tar.gz/.tgz, nested) has the bytes of an SDK file or a path through `epoc32/`; exit 1 on a leak, 2 when the check cannot be made (bzip2/xz tars included) |
| `serve <dir>` (hidden) | install.sh.test | the fake bucket: 127.0.0.1, prints its port, logs each request |

The tests are the Python tools' tests, case for case, plus their own; each tool's output
was compared with the Python tool's on the real inputs before the Python was removed
(NOTES-wip.md, "Rust tools"). pkgtools takes `symdev-elf2e32` from symdev by path until
symdev v0.3.0 is tagged (see `pkgtools/Cargo.toml`); until then only a machine with that
checkout builds this workspace.

## install.sh

`install.sh` (POSIX `sh`) installs the newest prebuilt symdev from the public bucket, where
symdev.yml serves it too, at the root, as `text/plain; charset=utf-8` with
`Cache-Control: no-cache`:

```bash
curl -fsSL https://pub-15670d2771364287b9982e497c29f586.r2.dev/install.sh | sh
```

It takes the highest `symdev;<ver>` with an `x86_64-linux` archive in `index.toml`, checks
its SHA-256 and size, extracts it into `$SYMDEV_HOME/symdev/<ver>/` (default
`~/.local/share/symdev`) with the receipt symdev writes (`symdev sdk list` shows it) and
links `~/.local/bin/symdev`. Re-running it updates; `SYMDEV_INSTALL_URL` points it at
another bucket. With OpenSSL 3 it first verifies the index's signature with the project's
public key, written into the script, and refuses an index that is unsigned or does not
verify; without OpenSSL 3 it warns that the index could not be verified and goes on (HTTPS
and SHA-256 still apply). `SYMDEV_INSTALL_PUBKEY` replaces the key (base64 Ed25519 public
keys, space-separated), for tests and for mirrors signed with their own key. `sh
tests/install.sh.test [<shell>]` runs it against a local fake bucket (`pkgtools serve`,
index and archives made by `publish --dry-run`, signed with a throwaway key; openssl 3
re-signs what a test edits); it builds `publish` and `pkgtools` itself.

## CI

`build.yml` (pull requests) builds GCCE in an AlmaLinux 8 container and packs it with
`publish public … --dry-run`; `publish.yml` (push to `main` under `recipes/gcce/`, or by
hand) does the same and uploads. `symdev.yml` does both for `recipes/symdev/<ver>/`: it
builds `publish` and `pkgtools` once (`$PUBLISH`, `$PKGTOOLS` for the later steps), builds
a static symdev (musl) from the recipe's tag, dry-runs both packages on every run,
and on `main` (or by hand, with the version) uploads `rust-sdk;<ver>`, then `symdev;<ver>`,
then `install.sh`; a manual run with `install_sh_only` uploads only `install.sh`. For a
recipe with `prebuilt.sh` (0.3.0 on) the build job also writes a `sources.toml` with the
private source (`key = "builtin"`), installs `gcce;12.1.0` and `sdk;s60-3rd-fp2;1.1` under
`$RUNNER_TEMP` with the symdev it just built (the only step given the reader key), runs
`prebuilt.sh`, and before the upload checks with `pkgtools sdk-free` that no file of the
artifact, nor of an archive in it, has the bytes of an SDK file or a path through `epoc32/`.
The artifact is `$RUNNER_TEMP/artifact` alone: the build's tar and the packed archives;
nothing is cached. The symdev
package holds `bin/symdev` and `share/doc/symdev/{LICENSE,THIRD-PARTY-NOTICES.txt}`, the
notices of every crate, bundled C library and toolchain runtime the static binary links
(`pkgtools notices`; build.sh fails without them). `tests.yml` runs
`tests/install.sh.test` under dash and bash and `cargo test` (publish and pkgtools) on
changes to what they cover; no step needs Python. Settings: repository variable `PUBLIC_READ_URL` (the
public bucket's r2.dev URL, read by dry runs); environment `publish` with variable
`PUBLISH_PUBLIC_URL` and secrets `PUBLISH_ACCESS_KEY_ID`, `PUBLISH_SECRET_ACCESS_KEY` and
`PUBLISH_SIGNING_KEY`, the last given only to the steps that upload an index; for
`prebuilt.sh`, repository variable `SYMDEV_PRIVATE_SOURCE_URL` (the private bucket's S3
URL) and repository secrets `SYMDEV_SOURCE_PRIVATE_ACCESS_KEY_ID` and
`SYMDEV_SOURCE_PRIVATE_SECRET_ACCESS_KEY` (its reader key).
