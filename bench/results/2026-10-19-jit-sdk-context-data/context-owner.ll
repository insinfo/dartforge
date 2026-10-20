@dartforge_context_slot = external global i64, align 8
declare ptr @dartforge_context()
declare i32 @dartforge_init(ptr, ptr)
define void @dartforge_entry() {
  ret void
}
define i64 @dartforge_dispatch_toString(i64 %value) {
  ret i64 0
}
define i32 @main() {
  %code = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)
  %expected = call ptr @dartforge_context()
  %offset = load i64, ptr @dartforge_context_slot, align 8
  %thread = call i64 asm "mrs $0, TPIDRRO_EL0", "=r"() nounwind memory(none)
  %base = and i64 %thread, -8
  %address = add i64 %base, %offset
  %slot = inttoptr i64 %address to ptr
  %actual = load ptr, ptr %slot, align 8
  %same = icmp eq ptr %actual, %expected
  %result = select i1 %same, i32 0, i32 1
  ret i32 %result
}
