@pragma('weak-tearoff-reference')
Function? weak(Function? value) => value;
@pragma('weak-tearoff-reference')
T Function()? genericWeak<T>(T Function()? value) => value;
class References {
  @pragma('weak-tearoff-reference')
  static Function? weak(Function? value) => value;
}
int first() => 11;
int unused() => 99;
class Methods {
  static int live() => 22;
  static int dead() => 33;
}
extension Callbacks on Object {
  static int live() => 44;
  static int dead() => 55;
}
const firstAlias = first;
const methodAlias = Methods.live;
const ordinary = weak;
Function? cycleA() => weak(cycleB);
Function? cycleB() => weak(cycleA);
void check(bool value, String message) {
  if (!value) throw StateError(message);
}
void main() {
  const local = first;
  final a = genericWeak<int>(firstAlias);
  final b = References.weak(methodAlias);
  final c = weak(Callbacks.live);
  check(a != null && a() == 11, 'generic wrapper');
  check(b != null && b() == 22, 'static wrapper');
  check(c != null && c() == 44, 'extension static target');
  check(identical(a, first) && identical(a, local), 'top-level identity');
  check(identical(b, Methods.live), 'static identity');
  check(identical(c, Callbacks.live), 'extension identity');
  check(a.hashCode == first.hashCode, 'canonical hash');
  check(a is int Function() && a is! String Function(), 'weak RTI');
  check(a.runtimeType == first.runtimeType, 'canonical RTI');
  check(weak(Methods.dead) == null, 'unused static');
  check(weak(Callbacks.dead) == null, 'unused extension static');
  check(weak(cycleA) == null && weak(cycleB) == null, 'weak-only cycle');
  check(weak(unused) != null, 'ordinary alias keeps its argument live');
  check(ordinary(unused) != null, 'constant wrapper alias is ordinary');
  check(first() + Methods.live() + Callbacks.live() == 77, 'strong calls');
  print('advanced weak contracts ok');
}
