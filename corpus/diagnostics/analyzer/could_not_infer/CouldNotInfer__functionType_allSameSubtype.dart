external T f<T extends num>(T a, T b);
void g(int cb(int a, int b)) {}
void main() {
  g(f);
}
