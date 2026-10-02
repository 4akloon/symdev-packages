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
.github/workflows/                       # build.yml (PRs, no upload), publish.yml (main), symdev.yml
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
```

`publish` packs the package reproducibly into `<sha256>.tar.gz` in the current directory,
checks the hash against the recipe, reads the bucket's `index.toml` (none yet = empty),
refuses an id that is already there, then uploads the archive (and for `public` the source
archive) and the index last. `--dry-run` uploads nothing and prints the index it would
write.

| Variable | For |
|---|---|
| `PUBLISH_PRIVATE_URL`, `PUBLISH_PUBLIC_URL` | the buckets' S3 endpoints, e.g. `https://<account>.r2.cloudflarestorage.com/symdev-private/` |
| `PUBLISH_ACCESS_KEY_ID`, `PUBLISH_SECRET_ACCESS_KEY` | the publisher key (R2 object read & write) |

A dry run needs none of them; with the bucket URL set it also reads the current index.

The SDK is published by the owner from their own copy:

```bash
cargo run --release -p publish -- private 'sdk;s60-3rd-fp2;1.1' \
  --from ~/sdk/S60_3rd_FP2 --recipe recipes/sdk/s60-3rd-fp2/1.1/recipe.toml
```

## install.sh

`install.sh` (POSIX `sh`) installs the newest prebuilt symdev from the public bucket, where
it is meant to be served too (`Cache-Control: no-cache`):

```bash
curl -fsSL https://pub-15670d2771364287b9982e497c29f586.r2.dev/install.sh | sh
```

It takes the highest `symdev;<ver>` with an `x86_64-linux` archive in `index.toml`, checks
its SHA-256 and size, extracts it into `$SYMDEV_HOME/symdev/<ver>/` (default
`~/.local/share/symdev`) with the receipt symdev writes (`symdev sdk list` shows it) and
links `~/.local/bin/symdev`. Re-running it updates; `SYMDEV_INSTALL_URL` points it at
another bucket. `sh tests/install.sh.test [<shell>]` runs it against a local fake bucket
(python3's `http.server`, index and archives made by `publish --dry-run`).

## CI

`build.yml` (pull requests) builds GCCE in a Debian 11 container and packs it with
`publish public … --dry-run`; `publish.yml` (push to `main` under `recipes/gcce/`, or by
hand) does the same and uploads. `symdev.yml` does both for `recipes/symdev/<ver>/`: it
builds a static symdev (musl) from the recipe's tag, dry-runs both packages on every run,
and on `main` (or by hand, with the version) uploads `rust-sdk;<ver>`, then `symdev;<ver>`. Settings: repository variable `PUBLIC_READ_URL` (the
public bucket's r2.dev URL, read by dry runs); environment `publish` with variable
`PUBLISH_PUBLIC_URL` and secrets `PUBLISH_ACCESS_KEY_ID`, `PUBLISH_SECRET_ACCESS_KEY`.
