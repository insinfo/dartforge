@pragma('weak-tearoff-reference')
Function? weak(Function? value) => value;
class C { C(); C.named(); }
class Empty {}
void check(bool value, String message) { if (!value) throw StateError(message); }
void main() {
  check(weak(C.new) == null, 'default constructor is only weak');
  check(weak(Empty.new) == null, 'synthetic constructor is only weak');
  C.named();
  final live = weak(C.named);
  check(live != null && live() is C, 'live named constructor');
  check(identical(live, C.named), 'canonical constructor identity');
  check(live is C Function(), 'constructor RTI');
  check(weak(C.new) == null, 'another live constructor must not retain the default');
  print('constructor weak contracts ok');
}
