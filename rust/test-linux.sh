#!/usr/bin/env bash
# Functional test of the Rust version on Linux (also works on macOS). Put this file next to the program
# "claude-vivarium-rs" (or run it from rust/ after "cargo build --release") and start it in a terminal
# inside the desktop session. Nothing is installed and the screen is not locked.
# Results: the folder "vivarium-test" next to this file (test.log and PNG files).
set -u
cd "$(dirname "$0")"
EXE=./claude-vivarium-rs
[ -x "$EXE" ] || EXE=./target/release/claude-vivarium-rs
[ -x "$EXE" ] || { echo "Program not found next to this script"; exit 1; }
OUT="$PWD/vivarium-test"
mkdir -p "$OUT"
LOG="$OUT/test.log"
{ echo "Claude Vivarium test $(date)"; uname -a; echo "session: ${XDG_SESSION_TYPE:-?} ${XDG_CURRENT_DESKTOP:-?}"; } > "$LOG"

step() { echo "$1"; echo "===== $2" >> "$LOG"; shift 2; "$@" >> "$LOG" 2>&1; echo "exit code $?" >> "$LOG"; }

step "[1/6] self test (fonts, sessions, graphics, PNG of every theme) ..." "selftest" \
    "$EXE" selftest --out "$OUT/themes"
step "[2/6] window for 6 seconds ..." "windowed" \
    env VIVARIUM_TEST="6;$OUT/windowed" "$EXE" run --windowed --demo
step "[3/6] full screen for 8 seconds, without locking - do not touch mouse or keyboard ..." "full screen" \
    env VIVARIUM_TEST="8;$OUT/full" "$EXE" run --demo --no-lock
step "[4/6] benchmark in a window, 20 seconds ..." "bench" \
    "$EXE" bench --theme synthwave --seconds 20
step "[5/6] benchmark full screen, 20 seconds - do not touch mouse or keyboard ..." "bench full screen" \
    "$EXE" bench --theme synthwave --seconds 20 --fullscreen
step "[6/6] benchmark with the CPU renderer, 15 seconds ..." "bench cpu" \
    "$EXE" bench --theme synthwave --seconds 15 --renderer cpu
echo "===== done" >> "$LOG"
echo
echo "Done. Results are in $OUT"
