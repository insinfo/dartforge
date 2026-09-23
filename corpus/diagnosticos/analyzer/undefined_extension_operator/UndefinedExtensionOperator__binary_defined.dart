extension E on String {
  void operator +(int offset) {}
}
f() {
  E('a') + 1;
}
