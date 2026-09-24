enum E { one, two, three }

void f(E e) {
  switch (e) {
//^^^^^^^^^^
// [diag.missingEnumConstantInSwitch] Missing case clause for 'three'.
    case E.one:
    case E.two:
      break;
  }
}
