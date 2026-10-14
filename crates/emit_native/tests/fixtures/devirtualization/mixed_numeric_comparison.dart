// Relational operators round mixed integers to double; compareTo is exact.
@pragma('vm:never-inline')
void compare(String label, dynamic left, dynamic right) {
  print('$label:${left == right},${left < right},${left <= right},'
      '${left > right},${left >= right},${left.compareTo(right)}');
}

void main() {
  compare('above53', 9007199254740993, 9007199254740992.0);
  compare('above53-reverse', 9007199254740992.0, 9007199254740993);
  compare('below53', -9007199254740993, -9007199254740992.0);
  compare('below53-reverse', -9007199254740992.0, -9007199254740993);
  compare('max', 9223372036854775807, 9223372036854775808.0);
  compare('max-reverse', 9223372036854775808.0, 9223372036854775807);
  compare('min', -9223372036854775808, -9223372036854775808.0);
  compare('min-reverse', -9223372036854775808.0, -9223372036854775808);
  compare('negative-zero', 0, -0.0);
  compare('negative-zero-reverse', -0.0, 0);
  compare('fraction', 1, 1.5);
  compare('fraction-reverse', 1.5, 1);
  compare('positive-infinity', 9223372036854775807, double.infinity);
  compare('negative-infinity', -9223372036854775808, double.negativeInfinity);
  compare('nan', 0, double.nan);
  compare('nan-reverse', double.nan, 0);
}
