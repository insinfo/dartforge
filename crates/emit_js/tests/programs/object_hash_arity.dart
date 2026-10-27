// `Object.hash` called with each arity agrees with the same call through
// a tear-off, which runs the original (sentinel-dispatching) body.
void check(int n, int direct, List<Object?> args) {
  final viaTearOff = Function.apply(Object.hash, args) as int;
  print('$n ${direct == viaTearOff}');
}

void main() {
  final v = <Object?>['a', 2, 3.5, null, true, #s, 'x', 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
  check(2, Object.hash(v[0], v[1]), v.sublist(0, 2));
  check(3, Object.hash(v[0], v[1], v[2]), v.sublist(0, 3));
  check(4, Object.hash(v[0], v[1], v[2], v[3]), v.sublist(0, 4));
  check(5, Object.hash(v[0], v[1], v[2], v[3], v[4]), v.sublist(0, 5));
  check(6, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5]), v.sublist(0, 6));
  check(7, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6]), v.sublist(0, 7));
  check(8, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]), v.sublist(0, 8));
  check(9, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]), v.sublist(0, 9));
  check(10, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9]), v.sublist(0, 10));
  check(11, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10]), v.sublist(0, 11));
  check(12, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11]), v.sublist(0, 12));
  check(13, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12]), v.sublist(0, 13));
  check(14, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13]), v.sublist(0, 14));
  check(15, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13], v[14]), v.sublist(0, 15));
  check(16, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13], v[14], v[15]), v.sublist(0, 16));
  check(17, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13], v[14], v[15], v[16]), v.sublist(0, 17));
  check(18, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13], v[14], v[15], v[16], v[17]), v.sublist(0, 18));
  check(19, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13], v[14], v[15], v[16], v[17], v[18]), v.sublist(0, 19));
  check(20, Object.hash(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10], v[11], v[12], v[13], v[14], v[15], v[16], v[17], v[18], v[19]), v.sublist(0, 20));
  print(Object.hash(1, 2) == Object.hash(1, 2));
  print(Object.hash(1, 2) == Object.hash(2, 1));
}
