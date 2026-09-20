bool unused() { print(999); return true; }
void promoted(int? value) {
  if (value != null) { print(value ?? unused()); }
}
void main() {
  print(1 ?? unused());
  int? number = 42;
  print(number ?? unused());
  promoted(8);
  promoted(null);
  bool? flag = false;
  print(flag ?? 7);
}
