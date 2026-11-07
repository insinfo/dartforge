@pragma('weak-tearoff-reference')
Function? weak(Function? value) => value;

int deadIf() => 1;
int deadConditional() => 2;
int liveIf() => 3;
int liveConditional() => 4;

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void main() {
  if (false) deadIf();
  final unused = false ? deadConditional() : 0;
  check(weak(deadIf) == null, 'literal false if target');
  check(weak(deadConditional) == null, 'literal false conditional target');
  if (true) liveIf();
  final result = true ? liveConditional() : 0;
  check(weak(liveIf) != null, 'literal true if target');
  check(weak(liveConditional) != null && result == 4, 'literal true conditional target');
  check(unused == 0, 'dead branch result');
  print('dead weak conditions ok');
}
