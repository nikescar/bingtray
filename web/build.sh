#!/usr/bin/env bash
set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/.."

# Pre-requisites:
rustup target add wasm32-unknown-unknown

# For generating JS bindings:
if ! cargo install --list | grep -q 'wasm-bindgen-cli v0.2.100'; then
    cargo install --force --quiet wasm-bindgen-cli --version 0.2.100
fi

CRATE_NAME="bingtray-core"
FEATURES="web_app"
OPEN=false
OPTIMIZE=false
BUILD=debug
BUILD_FLAGS=""
WGPU=false
WASM_OPT_FLAGS="-O2 --fast-math"

OUT_FILE_NAME="bingtray_core"

if [[ "${WGPU}" == true ]]; then
  FEATURES="${FEATURES},wgpu"
else
  FEATURES="${FEATURES},glow"
fi
FEATURES=""

FINAL_WASM_PATH=web/${OUT_FILE_NAME}.wasm

# Clear output from old stuff:
rm -f "${FINAL_WASM_PATH}"

echo "Building rust…"

(cd $CRATE_NAME &&
  cargo build \
    ${BUILD_FLAGS} \
    --quiet \
    --lib \
    --target wasm32-unknown-unknown \
    --no-default-features \
    # --features ${FEATURES}
)

# Get the output directory (in the workspace it is in another location)
# TARGET=`cargo metadata --format-version=1 | jq --raw-output .target_directory`
TARGET="target"

echo "Generating JS bindings for wasm…"
TARGET_NAME="${OUT_FILE_NAME}.wasm"
WASM_PATH="${TARGET}/wasm32-unknown-unknown/$BUILD/$TARGET_NAME"
wasm-bindgen "${WASM_PATH}" --out-dir web --out-name ${OUT_FILE_NAME} --no-modules --no-typescript

# if this fails with "error: cannot import from modules (`env`) with `--no-modules`", you can use:
# wasm2wat target/wasm32-unknown-unknown/release/egui_demo_app.wasm | rg env
# wasm2wat target/wasm32-unknown-unknown/release/egui_demo_app.wasm | rg "call .now\b" -B 20 # What calls `$now` (often a culprit)
# Or use https://rustwasm.github.io/twiggy/usage/command-line-interface/paths.html#twiggy-paths

# to get wasm-strip:  apt/brew/dnf install wabt
# wasm-strip ${FINAL_WASM_PATH}

if [[ "${OPTIMIZE}" = true ]]; then
  echo "Optimizing wasm…"
  # to get wasm-opt:  apt/brew/dnf install binaryen
  wasm-opt "${FINAL_WASM_PATH}" $WASM_OPT_FLAGS -o "${FINAL_WASM_PATH}"
fi

echo "Finished ${FINAL_WASM_PATH}"

# if [[ "${OPEN}" == true ]]; then
#   if [[ "$OSTYPE" == "linux-gnu"* ]]; then
#     # Linux, ex: Fedora
#     xdg-open http://localhost:8765/index.html
#   elif [[ "$OSTYPE" == "msys" ]]; then
#     # Windows
#     start http://localhost:8765/index.html
#   else
#     # Darwin/MacOS, or something else
#     open http://localhost:8765/index.html
#   fi
# fi