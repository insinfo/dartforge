extension _A on String {
  String operator -(int other) => this;
}
void f(String s) {
  s -= 3;
}
