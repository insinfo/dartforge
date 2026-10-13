enum E { one, two }

void f(E? e) {
  switch (e) {
//^^^^^^^^^^
// [diag.missingEnumConstantInSwitch] Missing case clause for 'null'.
    case E.one:
    case E.two:
      break;
  }
}
