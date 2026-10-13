external T f<T extends num>(T a, T b);
void g(num cb(int a, int b)) {}
void main() {
  g(f);
}
