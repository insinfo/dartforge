define i64 @bump(ptr %context) {
entry:
  %top_slot = getelementptr inbounds i8, ptr %context, i64 80
  %top = load ptr, ptr %top_slot, align 8
  %next = getelementptr inbounds i8, ptr %top, i64 24
  store ptr %next, ptr %top_slot, align 8
  store i64 1288490188801, ptr %top, align 8
  %field = getelementptr inbounds i8, ptr %top, i64 16
  store i64 0, ptr %field, align 8
  %address = ptrtoint ptr %top to i64
  %reference = add i64 %address, 2
  ret i64 %reference
}
define i64 @wrapper(ptr %context) {
  %object = call i64 @bump(ptr %context)
  ret i64 %object
}
