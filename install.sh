#!/bin/sh
# Install the newest prebuilt symdev (symdev spec 2026-10-02 §12):
#
#   curl -fsSL https://pub-15670d2771364287b9982e497c29f586.r2.dev/install.sh | sh
#
# Reads the public bucket's index.toml, takes the highest symdev;<version> that has an
# archive for this host, downloads it, checks its SHA-256 and size, extracts it into
# $SYMDEV_HOME/symdev/<version>/ (default ~/.local/share/symdev) with the receipt symdev
# itself writes, so `symdev sdk list` shows it, and links ~/.local/bin/symdev to it.
# Re-running it installs a newer version if there is one. It touches nothing else.
#
# SYMDEV_INSTALL_URL replaces the bucket: the URL of the directory that holds index.toml.
# Needs curl or wget, sha256sum or shasum, tar with gzip, and a POSIX sh.
set -eu

# Everything runs from main, called on the last line, so a download cut short (curl | sh)
# runs nothing.

say() { printf 'symdev install: %s\n' "$*"; }
warn() { printf 'symdev install: warning: %s\n' "$*" >&2; }
die() {
  printf 'symdev install: error: %s\n' "$*" >&2
  exit 1
}

main() {
  base=${SYMDEV_INSTALL_URL:-https://pub-15670d2771364287b9982e497c29f586.r2.dev/}
  case $base in */) ;; *) base=$base/ ;; esac

  [ -n "${HOME:-}" ] || die "HOME is not set"
  system=$(uname -sm)
  case $system in
    "Linux x86_64") host=x86_64-linux ;;
    *) die "symdev is prebuilt only for x86_64 Linux, and this is $system; build it from source: cargo install --git https://github.com/4akloon/symdev symdev-cli" ;;
  esac

  if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL -o "$2" "$1"; }
  elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q -O "$2" "$1"; }
  else
    die "neither curl nor wget is installed; install one of them"
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    sha256() { sha256sum "$1" | cut -d ' ' -f 1; }
  elif command -v shasum >/dev/null 2>&1; then
    sha256() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
  else
    die "neither sha256sum nor shasum is installed; install one of them"
  fi

  home=${SYMDEV_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/symdev}
  bindir=$HOME/.local/bin
  link=$bindir/symdev
  tmp=$(mktemp -d)
  staging=
  trap 'rm -rf "$tmp" ${staging:+"$staging"}' EXIT
  trap 'exit 1' HUP INT TERM

  # --- the index -------------------------------------------------------------------------
  index=${base}index.toml
  fetch "$index" "$tmp/index.toml" || die "cannot download $index"

  # One line per archive: `<id> <host> <url> <sha256> <size>`, after a `schema <n>` line. The
  # index is the publisher's TOML (`key = value` lines, [[package]] / [[package.archive]]).
  awk '
    function value(line) {
      sub(/^[^=]*=[ \t]*/, "", line); sub(/[ \t]+$/, "", line)
      if (line ~ /^".*"$/) line = substr(line, 2, length(line) - 2)
      return line
    }
    function flush() {
      if (archive && id != "") print id, host, url, sha, size
      archive = 0; host = url = sha = size = ""
    }
    /^[ \t]*(#|$)/ { next }
    /^\[\[package\]\][ \t]*$/ { flush(); id = ""; packages = 1; next }
    /^\[\[package\.archive\]\][ \t]*$/ { flush(); archive = 1; next }
    /^\[/ { flush(); next }
    {
      key = $0; sub(/[ \t]*=.*/, "", key); sub(/^[ \t]+/, "", key)
      if (key == "schema" && !packages) schema = value($0)
      else if (key == "id" && !archive) id = value($0)
      else if (archive && key == "host") host = value($0)
      else if (archive && key == "url") url = value($0)
      else if (archive && key == "sha256") sha = value($0)
      else if (archive && key == "size") size = value($0)
    }
    END { flush(); print "schema", schema }
  ' "$tmp/index.toml" >"$tmp/archives" || die "cannot read $index"
  schema=$(sed -n 's/^schema //p' "$tmp/archives")
  [ "$schema" = 1 ] ||
    die "$index has schema ${schema:-(none)}, which this install.sh does not read; download the current one from ${base}install.sh"

  # The highest symdev;<version> with an archive for this host. Versions compare by their
  # dot-separated numbers; a pre-release (`1.0.0-rc.1`) sorts before its release.
  awk -v host="$host" '
    function cmp(a, b,   pa, pb, ca, cb, na, nb, i, x, y) {
      pa = pb = ""
      if (i = index(a, "-")) { pa = substr(a, i + 1); a = substr(a, 1, i - 1) }
      if (i = index(b, "-")) { pb = substr(b, i + 1); b = substr(b, 1, i - 1) }
      na = split(a, ca, "."); nb = split(b, cb, ".")
      for (i = 1; i <= na || i <= nb; i++) {
        x = i <= na ? ca[i] : 0; y = i <= nb ? cb[i] : 0
        if (x ~ /^[0-9]+$/ && y ~ /^[0-9]+$/) { if (x + 0 != y + 0) return x + 0 < y + 0 ? -1 : 1 }
        else if (x != y) return x < y ? -1 : 1
      }
      if (pa == pb) return 0
      if (pa == "") return 1
      if (pb == "") return -1
      return pa < pb ? -1 : 1
    }
    $1 ~ /^symdev;/ && $2 == host {
      v = substr($1, 8)
      if (best == "" || cmp(v, best) > 0) { best = v; line = v " " $3 " " $4 " " $5 }
    }
    END { if (line != "") print line }
  ' "$tmp/archives" >"$tmp/chosen"
  read -r version url sha size <"$tmp/chosen" || die "no symdev for $host in $index"

  case $version in *[!0-9A-Za-z.+-]* | "") die "$index names symdev;$version, which is not a version" ;; esac
  case $sha in *[!0-9a-f]* | "") die "symdev;$version: sha256 '$sha' in $index is not lowercase hex" ;; esac
  [ ${#sha} -eq 64 ] || die "symdev;$version: sha256 '$sha' in $index is not 64 hex digits"
  case $size in *[!0-9]* | "") die "symdev;$version: size '$size' in $index is not a number" ;; esac
  # As symdev resolves archive URLs: a path under the index's directory, nothing else.
  case /$url/ in
    //* | *://* | *[\\?#\"]* | */./* | */../* | *//*)
      die "symdev;$version: archive URL '$url' in $index is not a path under its directory" ;;
  esac
  case $url in *[!!-~]*) die "symdev;$version: archive URL '$url' in $index has whitespace or a control character" ;; esac
  archive_url=$base$url

  dir=$home/symdev/$version
  receipt=$dir/.symdev-package.toml
  target=$dir/bin/symdev

  # --- install ---------------------------------------------------------------------------
  mkdir -p "$home"
  # The lock symdev takes for every install (flock on $SYMDEV_HOME/.lock), when flock(1)
  # exists; symdev clears $SYMDEV_HOME/.staging while it holds it.
  if command -v flock >/dev/null 2>&1; then
    exec 9>"$home/.lock"
    flock 9 || die "cannot lock $home/.lock"
  fi

  if [ -f "$receipt" ] && grep -qx "id = \"symdev;$version\"" "$receipt"; then
    say "symdev $version is already installed in $dir"
  else
    say "downloading symdev $version ($size bytes) from $archive_url"
    fetch "$archive_url" "$tmp/archive.tar.gz" || die "cannot download $archive_url"
    actual=$(sha256 "$tmp/archive.tar.gz")
    actual_size=$(wc -c <"$tmp/archive.tar.gz" | tr -d ' ')
    if [ "$actual" != "$sha" ] || [ "$actual_size" != "$size" ]; then
      die "symdev;$version: SHA-256 or size mismatch for $archive_url: the index says $sha ($size bytes), the download is $actual ($actual_size bytes); nothing was installed"
    fi
    tar -tzf "$tmp/archive.tar.gz" >"$tmp/entries" || die "$archive_url is not a .tar.gz"
    if grep -q -E -e '^/' -e '(^|/)\.\.(/|$)' "$tmp/entries"; then
      die "$archive_url has an entry outside the package: $(grep -E -e '^/' -e '(^|/)\.\.(/|$)' "$tmp/entries" | head -n 1)"
    fi
    staging=$home/.staging/install-sh-$$
    rm -rf "$staging"
    mkdir -p "$staging"
    tar -xzf "$tmp/archive.tar.gz" -C "$staging" || die "cannot extract $archive_url"
    [ -f "$staging/bin/symdev" ] && [ -x "$staging/bin/symdev" ] ||
      die "$archive_url has no executable bin/symdev"
    # A package directory without a receipt is unfinished: it is replaced, as symdev does.
    rm -rf "$dir"
    mkdir -p "$home/symdev"
    mv "$staging" "$dir"
    staging=
    rmdir "$home/.staging" 2>/dev/null || true
    printf 'id = "symdev;%s"\nsha256 = "%s"\nsource = "public"\nurl = "%s"\n' \
      "$version" "$sha" "$archive_url" >"$receipt.partial"
    mv -f "$receipt.partial" "$receipt"
    say "installed symdev $version in $dir"
  fi

  # --- the link --------------------------------------------------------------------------
  if [ -L "$link" ]; then
    current=$(readlink "$link")
    case $current in
      "$home"/symdev/*/bin/symdev) ;;
      *) die "$link links to $current, which install.sh did not install; remove it and run install.sh again" ;;
    esac
  elif [ -e "$link" ]; then
    die "$link exists and is not a link; move it away and run install.sh again"
  fi
  if [ "$(readlink "$link" 2>/dev/null || true)" != "$target" ]; then
    mkdir -p "$bindir"
    ln -s "$target" "$bindir/.symdev.install-sh-$$"
    mv -f "$bindir/.symdev.install-sh-$$" "$link"
    say "linked $link -> $target"
  fi

  case :$PATH: in
    *:"$bindir":* | *:"$bindir/":*) ;;
    *) warn "$bindir is not on your PATH; add it, e.g. in ~/.profile: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
  esac
}

main "$@"
