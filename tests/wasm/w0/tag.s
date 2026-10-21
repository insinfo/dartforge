# The exception tag LLVM's `llvm.wasm.throw(i32 0, ptr)` throws: the runtime
# defines it (libunwind does for C++); the payload is one pointer.
.tagtype __cpp_exception i32
.globl __cpp_exception
__cpp_exception:
