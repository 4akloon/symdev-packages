# symdev-packages

Recipes and the publisher for the packages [symdev](https://github.com/4akloon/symdev)
installs by itself (`symdev sdk install`, or the first `symdev build`). Design: symdev's
`docs/superpowers/specs/2026-10-02-toolchain-manager-design.md`, §2 and §6.

This repository never holds a proprietary file: no SDK, firmware, `.sis`, `.sisx`, `.cer`
or `.key` bytes. The SDK recipe lists paths and pins a hash; the bytes stay on the owner's
machine and in the private bucket.

## Layout

```
recipes/gcce/12.1.0/recipe.toml          # sources + sha256; built by build.sh in CI
recipes/gcce/12.1.0/build.sh
recipes/sdk/s60-3rd-fp2/1.1/recipe.toml  # paths taken from the SDK, pinned archive sha256
recipes/symdev/0.1.0/recipe.toml         # symdev;0.1.0 + rust-sdk;0.1.0 from the tag v0.1.0
recipes/symdev/0.1.0/build.sh
install.sh                               # installs the newest symdev from the public bucket
publish/                                 # the publisher (Rust, on symdev-sdk)
tools/third_party_notices.py             # THIRD-PARTY-NOTICES.txt of the static symdev
tests/                                   # install.sh.test, the notices generator's tests
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

A build that makes several packages (the symdev release makes `symdev` and `rust-sdk` from
one tag) keeps `git`, `tag`, `build` and `[[source]]` at the top and gives each package a
`[[package]]` table with its own `id`, `license`, `host`, `include` and `sha256`. `publish`
takes the package named by its `<id>` argument and checks all of them.

## Publishing

```
publish private <id> --from <dir> --recipe <recipe.toml> [--dry-run]
publish public  <id> --from <prefix> --source-code <tar.gz> --recipe <recipe.toml> [--dry-run]
publish file <path> --to <key> --bucket public|private --content-type <type> \
             --cache-control <value> [--dry-run]
publish sign-index --bucket public|private [--dry-run]
```

`publish` packs the package reproducibly into `<sha256>.tar.gz` in the current directory,
checks the hash against the recipe, reads the bucket's `index.toml` (none yet = empty),
refuses an id that is already there, then uploads the archive (and for `public` the source
archive) and the index last. `--dry-run` uploads nothing and prints the index it would
write.

Every index is signed (symdev spec §14): its first line is `# symdev-signature: ed25519
<base64>`, the project key's Ed25519 signature of the rest, which symdev and `install.sh`
check with the public key they carry. `publish` signs the index it writes with
`PUBLISH_SIGNING_KEY` (an upload needs it; a dry run signs when it is set and says when it
is not), and extends only an index whose signature verifies: one that does not is refused,
and an unsigned one is refused by an upload (a dry run warns), since otherwise the next
publish would sign whatever someone holding the bucket's R2 key wrote. `publish
sign-index` signs the bucket's current index as it is, byte for byte, after checking that
any signature it has verifies and that it parses, and lists its archives first; it is the
one way to accept an unsigned index (the indexes published before 0.2.0), so read the list
of a `--dry-run` before running it for real.

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
tests/install.sh.test [<shell>]` runs it against a local fake bucket (python3's
`http.server`, index and archives made by `publish --dry-run`, signed with a throwaway
key; openssl 3 re-signs what a test edits).

## CI

`build.yml` (pull requests) builds GCCE in an AlmaLinux 8 container and packs it with
`publish public … --dry-run`; `publish.yml` (push to `main` under `recipes/gcce/`, or by
hand) does the same and uploads. `symdev.yml` does both for `recipes/symdev/<ver>/`: it
builds a static symdev (musl) from the recipe's tag, dry-runs both packages on every run,
and on `main` (or by hand, with the version) uploads `rust-sdk;<ver>`, then `symdev;<ver>`,
then `install.sh`; a manual run with `install_sh_only` uploads only `install.sh`. The symdev
package holds `bin/symdev` and `share/doc/symdev/{LICENSE,THIRD-PARTY-NOTICES.txt}`, the
notices of every crate, bundled C library and toolchain runtime the static binary links
(`tools/third_party_notices.py`; build.sh fails without them). `tests.yml` runs
`tests/install.sh.test` under dash and bash, the generator's tests and `cargo test` on
changes to what they cover. Settings: repository variable `PUBLIC_READ_URL` (the
public bucket's r2.dev URL, read by dry runs); environment `publish` with variable
`PUBLISH_PUBLIC_URL` and secrets `PUBLISH_ACCESS_KEY_ID`, `PUBLISH_SECRET_ACCESS_KEY` and
`PUBLISH_SIGNING_KEY`, the last given only to the steps that upload an index.
