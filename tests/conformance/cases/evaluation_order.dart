int mark(int n) {
  print(n);
  return n;
}
int add(int a, int b) {
  return a + b;
}
bool probe() {
  print('probe');
  return true;
}
void main() {
  print(add(mark(1), mark(2)));
  print(false && probe());
  print(true || probe());
  if (true) { print('then'); } else { print('unreachable'); }
}
