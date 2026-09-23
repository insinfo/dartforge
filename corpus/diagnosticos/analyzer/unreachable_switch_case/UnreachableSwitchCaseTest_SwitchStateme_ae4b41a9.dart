void f(Object? x) {
  switch (x) {
    case 0:
      break;
    case unresolved:
//       ^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'unresolved'.
      break;
    case 2:
      break;
  };
}
