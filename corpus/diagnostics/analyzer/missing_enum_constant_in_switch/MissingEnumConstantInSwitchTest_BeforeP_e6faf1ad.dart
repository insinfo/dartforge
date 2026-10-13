enum E { one, two, three }

void f(E e) {
  switch (e) {
//^^^^^^^^^^
// [diag.missingEnumConstantInSwitch] Missing case clause for 'two'.
    case E.one:
    case E.three:
      break;
  }
}
