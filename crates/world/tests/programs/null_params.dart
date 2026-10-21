// Parameters that every call leaves null (`params.rs`): the branches that
// test them are dead, and so is what only those branches call. Each `dead*`
// function is reached only through such a branch; each `live*` one through a
// counterexample.

void deadStatic() => print('deadStatic');
void deadCtor() => print('deadCtor');
void deadVirtual() => print('deadVirtual');
void deadAfterReturn() => print('deadAfterReturn');
void deadChained() => print('deadChained');
void liveTitle() => print('liveTitle');
void liveTearOff() => print('liveTearOff');
void liveDynamic() => print('liveDynamic');
void liveWritten() => print('liveWritten');
void liveDefault() => print('liveDefault');
void liveVirtual() => print('liveVirtual');

void staticOnly({String? tag}) {
  if (tag != null) deadStatic();
  print('static');
}

class Config {
  final String mode;
  Config({String? mode}) : mode = mode == null ? 'none' : _build(mode);
  static String _build(String m) {
    deadCtor();
    return m;
  }
}

abstract class Shape {
  String describe([String? suffix]);
}

class Square implements Shape {
  @override
  String describe([String? suffix]) {
    if (suffix == null) return 'square';
    deadVirtual();
    return 'square $suffix';
  }
}

class Circle implements Shape {
  @override
  String describe([String? suffix]) {
    if (suffix != null) deadVirtual();
    return 'circle';
  }
}

void early({Object? extra}) {
  if (extra == null) return;
  deadAfterReturn();
}

void outer({String? extra}) => inner(extra);
void inner(String? extra) {
  if (extra != null) deadChained();
}

void greet([String? title]) {
  if (title != null) liveTitle();
}

void escaped({String? tag}) {
  if (tag != null) liveTearOff();
}

class Widget {
  void render({String? prefix}) {
    if (prefix != null) liveDynamic();
  }
}

void written([int? x]) {
  x ??= 1;
  if (x != null) liveWritten();
}

void defaulted([int? x = 5]) {
  if (x != null) liveDefault();
}

abstract class Node {
  void label([String? mark]);
}

class Leaf extends Node {
  @override
  void label([String? mark]) {
    if (mark != null) liveVirtual();
  }
}

class Branch extends Node {
  @override
  void label([String? mark]) {}
}

void main() {
  staticOnly();
  staticOnly(tag: null);
  print(Config().mode);
  for (Shape s in [Square(), Circle()]) {
    print(s.describe());
  }
  early();
  outer();
  greet();
  greet('Dr.');
  var f = escaped;
  f(tag: 'x');
  dynamic w = Widget();
  w.render(prefix: 'p');
  written();
  defaulted();
  Node n = Branch();
  n.label('!');
  Leaf().label();
}
