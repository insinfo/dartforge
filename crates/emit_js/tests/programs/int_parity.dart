// `isEven` and `isOdd` of a non-nullable `int` are written inline in
// production; every value must give the VM's answer.

int calls = 0;

int counted(int v) {
  calls++;
  return v;
}

void main() {
  final values = [0, 1, -1, 2, -2, 7, -7, 2147483647, -2147483648, 2147483648,
      4294967295, 4294967296, 4294967297, 9007199254740991, -9007199254740991];
  for (final v in values) {
    print('$v ${v.isEven} ${v.isOdd} ${(-v).isEven} ${(v + 1).isOdd}');
  }
  print(counted(3).isOdd);
  print(calls);
  int? maybe = values.length > 2 ? 5 : null;
  print(maybe?.isEven);
  if (maybe != null) print(maybe.isOdd);
  num n = 4;
  if (n is int) print(n.isEven);
  dynamic d = 6;
  print(d.isEven);
  print([1, 2, 3, 4].where((i) => i.isOdd).toList());
}
