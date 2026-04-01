#!/usr/bin/env zsh

set -euo pipefail

repo="pguilbert/canopy"
bin_name="canopy"
install_dir="${INSTALL_DIR:-${HOME}/.local/bin}"
version="${1:-latest}"

case "$(uname -s)" in
  Darwin)
    os="apple-darwin"
    ;;
  Linux)
    os="unknown-linux-gnu"
    ;;
  *)
    print -u2 "error: unsupported operating system: $(uname -s)"
    exit 1
    ;;
esac

case "$(uname -m)" in
  arm64|aarch64)
    arch="aarch64"
    ;;
  x86_64)
    arch="x86_64"
    ;;
  *)
    print -u2 "error: unsupported architecture: $(uname -m)"
    exit 1
    ;;
esac

target="${arch}-${os}"
archive="${bin_name}-${target}.tar.gz"

if [[ "${version}" == "latest" ]]; then
  url="https://github.com/${repo}/releases/latest/download/${archive}"
else
  url="https://github.com/${repo}/releases/download/${version}/${archive}"
fi

if ! command -v curl >/dev/null 2>&1; then
  print -u2 "error: curl is not installed or not on PATH"
  exit 1
fi

if ! command -v tar >/dev/null 2>&1; then
  print -u2 "error: tar is not installed or not on PATH"
  exit 1
fi

mkdir -p "${install_dir}"
tmp_dir=$(mktemp -d)
trap 'rm -rf "${tmp_dir}"' EXIT

print "Downloading ${url}"
curl -fLsS "${url}" -o "${tmp_dir}/${archive}"
tar -xzf "${tmp_dir}/${archive}" -C "${tmp_dir}"
install -m 0755 "${tmp_dir}/${bin_name}" "${install_dir}/${bin_name}"

if [[ ":${PATH}:" != *":${install_dir}:"* ]]; then
  print
  print "${install_dir} is not on PATH for this shell."
  print "Add this line to ~/.zshrc if needed:"
  print "export PATH=\"${install_dir}:\$PATH\""
fi

print
print "Installed ${bin_name} to ${install_dir}/${bin_name}"
print "Try: ${bin_name} --help"
