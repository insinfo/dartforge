void f(List<Object> x) {
  var <int>[a] = x;
//    ^^^^^^^^
// [diag.patternTypeMismatchInIrrefutableContext] The matched value of type 'List<Object>' isn't assignable to the required type 'List<int>'.
//          ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
