void f() {
  // ignore:unused_local_variable
  late final int? x;
  x += 1;
//^
// [diag.definitelyUnassignedLateLocalVariable] The late local variable 'x' is definitely unassigned at this point.
//  ^^
// [diag.uncheckedMethodInvocationOfNullableValue] The method '+' can't be unconditionally invoked because the receiver can be 'null'.
}
