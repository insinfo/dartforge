extension _A on bool {
  int operator [](int index) => 7;
}
void main() {
  false[3];
}
