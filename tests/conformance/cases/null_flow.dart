bool valid(int? value) { return value != null && value > 0; }
bool empty(int? value) { return value == null || value < 1; }
int choose(int? value) {
  if (value != null) { return value + 2; } else { return 7; }
}
void main() {
  print(valid(null)); print(valid(3));
  print(empty(null)); print(empty(3));
  print(choose(null)); print(choose(4));
  int? value = 0;
  while (value != null) {
    print(value + 1);
    value = null;
  }
}
