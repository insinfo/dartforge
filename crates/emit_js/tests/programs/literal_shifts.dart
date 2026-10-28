// Shifts of an `int` by a literal count, which the production profile
// writes inline: every value must be what `JSNumber`'s operators give
// (the web semantics of 32-bit shifts, which differ from the VM for
// negative operands and counts above 31).

int calls = 0;

int next(int v) {
  calls++;
  return v;
}

void main() {
  final values = [0, 1, 5, 255, 1 << 20, 2147483647, 2147483648, 4294967295, -1, -2, -8, -2147483648];
  for (final v in values) {
    print([v << 0, v << 1, v << 5, v << 31, v << 32, v << 40]);
    print([v >> 0, v >> 1, v >> 5, v >> 31, v >> 32, v >> 40]);
    print([v >>> 0, v >>> 1, v >>> 5, v >>> 31, v >>> 32, v >>> 40]);
  }
  print(next(3) << 2);
  print(next(-3) >> 1);
  print(next(-3) >> 40);
  print(next(7) << 33);
  print(next(7) >>> 33);
  print(calls);
  var x = 12;
  x <<= 2;
  x >>= 1;
  x >>>= 1;
  print(x);
}
