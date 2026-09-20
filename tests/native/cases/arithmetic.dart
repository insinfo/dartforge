int square(int x) { return x * x; }
int fact(int n) { if (n <= 1) { return 1; } return n * fact(n - 1); }
void main() {
  print(fact(10)); print(square(-9));
  int large = 2147483647;
  print(large * large);
  print(large * large * 4 + large * 4 + 1);
  print(-(large * large * 4 + large * 4 + 1));
  print(true == false); print(7 < 8);
}
