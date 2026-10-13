class C {
  T f<T>(T t) => null;
//               ^^^^
// [diag.returnOfInvalidTypeFromMethod] A value of type 'Null' can't be returned from the method 'f' because it has a return type of 'T'.
}
main() { new C().f(<S>(S s) => s); }
