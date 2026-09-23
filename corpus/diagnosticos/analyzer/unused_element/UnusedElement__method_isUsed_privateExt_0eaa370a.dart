extension _A on bool {
  operator []=(int index, int value) {}
}
void main() {
  false[0] = 1;
}
