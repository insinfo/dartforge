int? optional(bool available) { if (available) { return 42; } }
void main() {
  int? value = optional(false);
  print(value);
  print(value ?? 7);
  value = optional(true);
  if (value != null) { print(value + 1); }
  print(value!);
}
