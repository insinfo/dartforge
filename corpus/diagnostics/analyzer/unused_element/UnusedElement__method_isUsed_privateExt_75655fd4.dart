extension _A on bool {
  int? operator [](int index) => 7;
  void operator []=(int index, int value) {}
}
void main() {
  false[3] ??= 1;
}
