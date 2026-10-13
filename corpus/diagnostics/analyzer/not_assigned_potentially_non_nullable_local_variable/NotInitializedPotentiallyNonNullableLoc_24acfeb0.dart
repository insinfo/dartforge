// %before-language-feature: patterns
void f(int e) {
  int v;
  switch (e) {
    L: case 1:
      v = 0;
      break;
    case 2:
      continue L;
    default:
      v = 0;
  }
  v;
}
