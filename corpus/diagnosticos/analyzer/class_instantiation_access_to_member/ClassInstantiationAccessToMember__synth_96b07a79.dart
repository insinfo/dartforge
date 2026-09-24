class A<T> {
  A.foo();
}

var x = A<int>.;
//             ^
// [diag.missingIdentifier] Expected an identifier.
