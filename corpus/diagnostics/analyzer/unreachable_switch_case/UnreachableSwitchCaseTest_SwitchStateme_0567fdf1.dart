void f(Object? x) {
  switch (x) {
    case 0:
      break;
    case Unresolved():
//       ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
      break;
    case 2:
      break;
  };
}
