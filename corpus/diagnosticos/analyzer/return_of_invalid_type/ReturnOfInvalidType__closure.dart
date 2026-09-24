typedef Td = int Function();
Td f() {
  return () => "hello";
//             ^^^^^^^
// [diag.returnOfInvalidTypeFromClosure] The returned type 'String' isn't returnable from a 'int' function, as required by the closure's context.
}
