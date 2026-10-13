// @dart=2.19
void f(bool b, int x) {
  switch (b) {
    case true:
      print(1);
      break;
    case false:
      print(2);
      break;
    case true:
      print(3);
      break;
  }
  switch (x) {
    case 'a':
      break;
  }
}
