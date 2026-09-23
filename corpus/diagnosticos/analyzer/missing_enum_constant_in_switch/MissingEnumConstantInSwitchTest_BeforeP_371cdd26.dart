enum E { one, two, three }

void f(E e) {
  switch (e) {
//^^^^^^^^^^
// [diag.missingEnumConstantInSwitch] Missing case clause for 'one'.
    case E.two:
    case E.three:
      break;
  }
}
