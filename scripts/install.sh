#!/bin/sh

set -eu

REPOSITORY="${RELORA_REPOSITORY:-murongg/relora}"
INSTALL_DIR="${RELORA_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="${RELORA_BASE_URL:-}"
VERSION="${RELORA_VERSION:-}"
TAG="${RELORA_TAG:-}"

main() {
  require_cmd curl
  require_cmd tar
  require_cmd mktemp
  require_cmd uname

  platform="$(detect_platform)"
  arch="$(detect_arch)"
  tag="$(resolve_tag)"
  version="${tag#v}"
  asset_name="relora-v${version}-${platform}-${arch}.tar.gz"
  download_url="$(resolve_download_url "$tag" "$asset_name")"

  tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/relora-install.XXXXXX")"
  trap 'rm -rf "$tmp_dir"' EXIT INT TERM

  archive_path="${tmp_dir}/${asset_name}"
  extract_dir="${tmp_dir}/extract"
  mkdir -p "$extract_dir" "$INSTALL_DIR"

  printf '%s\n' "Installing Relora ${version} for ${platform}-${arch}..."
  printf '%s\n' "Downloading ${download_url}"
  curl -fL# "$download_url" -o "$archive_path"

  tar -xzf "$archive_path" -C "$extract_dir"

  bundle_root="$extract_dir"
  set -- "$extract_dir"/*
  if [ "$#" -eq 1 ] && [ -d "$1" ]; then
    bundle_root="$1"
  fi

  for file_name in relora relora-driver-postgres relora-driver-mysql relora-driver-sqlite; do
    source_path="${bundle_root}/${file_name}"
    target_path="${INSTALL_DIR}/${file_name}"
    if [ ! -f "$source_path" ]; then
      printf '%s\n' "Downloaded bundle is missing ${file_name}." >&2
      exit 1
    fi
    cp "$source_path" "$target_path"
    chmod 755 "$target_path"
  done

  printf '\n%s\n' "Relora was installed to ${INSTALL_DIR}"
  case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      printf '%s\n' "Add ${INSTALL_DIR} to your PATH before running relora."
      ;;
  esac
  printf '%s\n' "Run: relora"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf '%s\n' "Missing required command: $1" >&2
    exit 1
  fi
}

detect_platform() {
  case "$(uname -s)" in
    Darwin) printf '%s' "darwin" ;;
    Linux) printf '%s' "linux" ;;
    *)
      printf '%s\n' "Relora install.sh currently supports macOS and Linux." >&2
      exit 1
      ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64) printf '%s' "x64" ;;
    arm64|aarch64) printf '%s' "arm64" ;;
    *)
      printf '%s\n' "Relora install.sh currently supports x64 and arm64." >&2
      exit 1
      ;;
  esac
}

resolve_tag() {
  if [ -n "$TAG" ]; then
    printf '%s' "$TAG"
    return
  fi

  if [ -n "$VERSION" ]; then
    printf 'v%s' "$VERSION"
    return
  fi

  latest_api="https://api.github.com/repos/${REPOSITORY}/releases/latest"
  latest_tag="$(
    curl -fsSL "$latest_api" \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -n 1
  )"

  if [ -z "$latest_tag" ]; then
    printf '%s\n' "Unable to detect the latest Relora release. Set RELORA_VERSION explicitly and retry." >&2
    exit 1
  fi

  printf '%s' "$latest_tag"
}

resolve_download_url() {
  tag="$1"
  asset_name="$2"

  if [ -n "$BASE_URL" ]; then
    printf '%s/%s' "${BASE_URL%/}" "$asset_name"
    return
  fi

  printf 'https://github.com/%s/releases/download/%s/%s' "$REPOSITORY" "$tag" "$asset_name"
}

main "$@"
