; Negative control of scripts/wasm-w0.sh: atomics over a shared memory are
; outside the wasm32-linear profile, and the gate must refuse the module LLVM
; and LLD make from this file.
target triple = "wasm32-unknown-unknown"

@counter = global i32 0

define i32 @dartforge_run() {
  %old = atomicrmw add ptr @counter, i32 1 seq_cst
  ret i32 %old
}
