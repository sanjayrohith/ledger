#!/usr/bin/env bash
# Ledger installer for macOS (Apple Silicon).
#
#   curl -fsSL https://raw.githubusercontent.com/sanjayrohith/ledger/main/install.sh | bash
#
# What it does, in order:
#   1. checks you are on an Apple Silicon Mac
#   2. installs anything missing: Xcode Command Line Tools, Rust, Node.js
#   3. downloads the source to ~/.ledger-src (or uses this folder if you ran it from a clone)
#   4. builds Ledger on your Mac and copies Ledger.app to /Applications
#   5. clears the macOS download quarantine flag (the app is unsigned) and opens it
#
# Nothing is sent anywhere. Your finance data is created later, locally, by the app itself.
# Environment overrides:
#   LEDGER_INSTALL_DIR  where Ledger.app is copied   (default: /Applications)
#   LEDGER_SRC_DIR      where the source is kept      (default: ~/.ledger-src)
#   LEDGER_NO_OPEN=1    do not open the app when done

set -euo pipefail

REPO_URL="https://github.com/sanjayrohith/ledger.git"
INSTALL_DIR="${LEDGER_INSTALL_DIR:-/Applications}"
SRC_DIR="${LEDGER_SRC_DIR:-$HOME/.ledger-src}"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
step() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
fail() { printf '\n\033[31mError:\033[0m %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- 1. Check the machine ------------------------------------------------------
step "Checking your Mac"
[ "$(uname -s)" = "Darwin" ] || fail "Ledger is a macOS app. This installer only runs on a Mac."
[ "$(uname -m)" = "arm64" ] || fail "Ledger is built for Apple Silicon (M1 or newer). This Mac reports $(uname -m)."
echo "macOS on Apple Silicon: ok"

# --- 2. Tools --------------------------------------------------------------------
step "Checking build tools"

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Xcode Command Line Tools are missing. A macOS install window will open."
  xcode-select --install >/dev/null 2>&1 || true
  echo "Waiting for the install to finish (this can take a few minutes)..."
  until xcode-select -p >/dev/null 2>&1; do sleep 5; done
fi
echo "Xcode Command Line Tools: ok"

if ! have git; then fail "git is missing even after installing the Command Line Tools. Run: xcode-select --install"; fi

if ! have cargo && [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi
if ! have cargo; then
  echo "Installing Rust (rustup, official installer)..."
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi
echo "Rust: $(rustc --version)"

if ! have node || ! have npm; then
  if have brew; then
    echo "Installing Node.js with Homebrew..."
    brew install node
  else
    fail "Node.js is missing. Install it from https://nodejs.org (the LTS version), open a new Terminal window, and run this command again."
  fi
fi
echo "Node.js: $(node --version)"

# --- 3. Source ----------------------------------------------------------------------
step "Getting the Ledger source"
SCRIPT_DIR=""
if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fi

if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/package.json" ] && [ -d "$SCRIPT_DIR/src-tauri" ]; then
  SRC_DIR="$SCRIPT_DIR"
  echo "Using this folder: $SRC_DIR"
elif [ -d "$SRC_DIR/.git" ]; then
  echo "Updating $SRC_DIR"
  git -C "$SRC_DIR" fetch --quiet origin main
  git -C "$SRC_DIR" reset --quiet --hard origin/main
else
  echo "Downloading to $SRC_DIR"
  git clone --quiet --depth 1 "$REPO_URL" "$SRC_DIR"
fi
cd "$SRC_DIR"

# --- 4. Build ---------------------------------------------------------------------------
step "Building Ledger (the first build takes 5 to 10 minutes)"
npm ci --no-audit --no-fund
npm run tauri build -- --bundles app

APP_SRC="$SRC_DIR/src-tauri/target/release/bundle/macos/Ledger.app"
[ -d "$APP_SRC" ] || fail "The build finished but Ledger.app was not found at $APP_SRC"

# --- 5. Install -------------------------------------------------------------------------
step "Installing Ledger.app into $INSTALL_DIR"
mkdir -p "$INSTALL_DIR" 2>/dev/null || true
if pgrep -x ledger >/dev/null 2>&1; then
  echo "Closing the running Ledger so it can be replaced..."
  osascript -e 'quit app "Ledger"' >/dev/null 2>&1 || true
  sleep 2
fi
TARGET="$INSTALL_DIR/Ledger.app"
if [ -d "$TARGET" ]; then
  rm -rf "$TARGET" 2>/dev/null || sudo rm -rf "$TARGET"
fi
if ! cp -R "$APP_SRC" "$INSTALL_DIR/" 2>/dev/null; then
  echo "Copying needs administrator permission."
  sudo cp -R "$APP_SRC" "$INSTALL_DIR/"
fi
# The app is built on this Mac and is not signed with an Apple Developer ID.
# Clearing the quarantine flag lets macOS open it without the "unidentified developer" block.
xattr -dr com.apple.quarantine "$TARGET" 2>/dev/null || true

bold ""
bold "Ledger is installed: $TARGET"
echo "Your data stays on this Mac. To update later, run the same command again."
echo "Your transactions are kept: updating replaces only the app, not your database."

if [ -z "${LEDGER_NO_OPEN:-}" ]; then
  open "$TARGET"
fi
