# Build tasks for gpui-cef.
#
# cef-dll-sys downloads the matching CEF distribution during the first build.
# Set CEF_PATH only to use an existing local CEF distribution.

# Check that the prerequisites are in place
doctor:
    @command -v c++ >/dev/null && echo "C++ compiler: ok" || echo "C++ compiler: missing"
    @command -v ninja >/dev/null && echo "ninja:       ok" || echo "ninja:       missing"
    @command -v cmake >/dev/null && echo "cmake:       ok" || echo "cmake:       missing — brew install cmake"

check:
    cargo check --workspace

build:
    cargo build -p gpui-cef-browser

# Assemble a platform bundle with the matching CEF runtime.
bundle:
    cargo run -p xtask

# Assemble and launch the platform bundle.
run:
    cargo run -p xtask -- run

# Bundle, then click through the demo's feature test page over CEF's remote
# debugging port and check each round trip. Opens a window, so it needs a
# desktop session; needs Node 22 or later.
e2e: bundle
    node examples/browser/e2e.mjs

# Real AppKit text-client callbacks in an isolated, invisible macOS bundle.
input-e2e: bundle
    node examples/browser/input-e2e.mjs

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets
