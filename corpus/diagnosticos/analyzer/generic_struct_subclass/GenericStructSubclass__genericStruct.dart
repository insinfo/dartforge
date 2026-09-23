import 'dart:ffi';
final class S<T> extends Struct {
//          ^
// [diag.genericStructSubclass] The class 'S' can't extend 'Struct' or 'Union' because 'S' is generic.
  external Pointer notEmpty;
}
