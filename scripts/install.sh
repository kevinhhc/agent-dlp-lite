#!/bin/sh
# Offline installer. Obtain the expected SHA-256 from a trusted release manifest.
set -eu
binary=''
expected=''
prefix="${HOME}/.local/bin"
force=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary) binary=$2; shift 2 ;;
    --sha256) expected=$2; shift 2 ;;
    --prefix) prefix=$2; shift 2 ;;
    --force) force=true; shift ;;
    *) echo 'Usage: install.sh --binary PATH --sha256 HASH [--prefix DIR] [--force]' >&2; exit 1 ;;
  esac
done
[ -f "$binary" ] || { echo 'A local binary is required' >&2; exit 1; }
[ "${#expected}" -eq 64 ] || { echo 'A 64-character SHA-256 is required' >&2; exit 1; }
case "$expected" in *[!a-fA-F0-9]*) echo 'Invalid SHA-256' >&2; exit 1 ;; esac
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$binary" | cut -d ' ' -f 1)
else
  actual=$(shasum -a 256 "$binary" | cut -d ' ' -f 1)
fi
expected=$(printf '%s' "$expected" | tr 'A-F' 'a-f')
[ "$actual" = "$expected" ] || { echo 'Checksum mismatch; nothing installed' >&2; exit 1; }
mkdir -p "$prefix"
target="$prefix/agent-dlp"
[ ! -L "$target" ] || { echo 'Refusing symlink destination' >&2; exit 1; }
if [ -e "$target" ]; then
  [ "$force" = true ] || { echo 'Already installed; use --force to replace' >&2; exit 1; }
  cp -p "$target" "$target.previous.$(date +%s).$$"
fi
temp=$(mktemp "$prefix/.agent-dlp.XXXXXX")
trap 'rm -f "$temp"' EXIT HUP INT TERM
cp "$binary" "$temp"
chmod 755 "$temp"
mv -f "$temp" "$target"
printf 'Installed: %s\nAdd this directory to PATH if needed. Run agent-dlp self-test.\n' "$target"
