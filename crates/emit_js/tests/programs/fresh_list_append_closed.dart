var effects = 0;
class Plain { final int id; Plain(this.id); }
class Stateful { final int id; Stateful(this.id) { effects++; } }
class Initialized { final int id; Initialized(int value) : id = compute(value); }
class DefaultField { final int id = compute(8); DefaultField(); }
class Base { Base() { effects++; } }
class Derived extends Base { final int id; Derived(this.id); }
class Metadata { final int $ti; Metadata(this.$ti); }
class Source { int get next { effects++; return 4; } }
class Operator { int operator +(int value) { effects++; return value; } }
int compute(int value) { effects++; return value; }
final source = Source();
final operatorValue = Operator();
List<Plain>? exposed;

List<Plain> pure(int count) {
  final values = <Plain>[];
  for (var i = 0; i < count; i++) { values.add(Plain(i & 255)); }
  return values;
}
List<Stateful> constructorBody(int count) {
  final values = <Stateful>[];
  for (var i = 0; i < count; i++) { values.add(Stateful(i)); }
  return values;
}
List<Initialized> initializerCall(int count) {
  final values = <Initialized>[];
  for (var i = 0; i < count; i++) { values.add(Initialized(i)); }
  return values;
}
List<DefaultField> implicitField(int count) {
  final values = <DefaultField>[];
  for (var i = 0; i < count; i++) { values.add(DefaultField()); }
  return values;
}
List<Derived> inheritedBody(int count) {
  final values = <Derived>[];
  for (var i = 0; i < count; i++) { values.add(Derived(i)); }
  return values;
}
List<Plain> globalGetter(int count) {
  final values = <Plain>[];
  for (var i = 0; i < count; i++) { values.add(Plain(source.next)); }
  return values;
}
List<Plain> customOperator(int count) {
  final values = <Plain>[];
  for (var i = 0; i < count; i++) { values.add(Plain(operatorValue + i)); }
  return values;
}
List<Plain> callback(int count, int Function(int) fn) {
  final values = <Plain>[];
  for (var i = 0; i < count; i++) { values.add(Plain(fn(i))); }
  return values;
}
List<Plain> globalEscape(int count) {
  final values = <Plain>[];
  exposed = values;
  for (var i = 0; i < count; i++) { values.add(Plain(i)); }
  return values;
}
List<Metadata> reservedMetadata(int count) {
  final values = <Metadata>[];
  for (var i = 0; i < count; i++) { values.add(Metadata(i)); }
  return values;
}
List<Plain> shadowedCounter(int count) {
  final values = <Plain>[];
  for (var i = 0; i < count; i++) {
    for (var i = 0; i < 1; i++) { values.add(Plain(i)); }
  }
  return values;
}
List<Plain> shadowedParameter(int i) {
  final values = <Plain>[];
  for (var i = 0; i < 2; i++) { values.add(Plain(i)); }
  return values;
}
List<Plain> alternateWrite(int count, dynamic replacement) {
  final values = <Plain>[];
  for (var i = 0; i < count; i++) {
    values.add(Plain(i));
    i = replacement;
  }
  return values;
}
List<Plain> unknownCounter(int count, dynamic next) {
  final values = <Plain>[];
  for (var i = 0; i < count; i = next + i) { values.add(Plain(i)); }
  return values;
}
List<Plain> dynamicOverflow(int count, dynamic delta) {
  final values = <Plain>[];
  dynamic value = 2147483647;
  for (var i = 0; i < count; i++) {
    value += delta;
    values.add(Plain(value & 255));
  }
  return values;
}
void main() {
  print(pure(3).map((v) => v.id).toList());
  print(constructorBody(3).map((v) => v.id).toList());
  print(initializerCall(3).map((v) => v.id).toList());
  print(implicitField(3).map((v) => v.id).toList());
  print(inheritedBody(3).map((v) => v.id).toList());
  print(globalGetter(3).map((v) => v.id).toList());
  print(customOperator(3).map((v) => v.id).toList());
  print(callback(3, compute).map((v) => v.id).toList());
  print(globalEscape(3).map((v) => v.id).toList());
  print(reservedMetadata(3).length);
  print(shadowedCounter(2).map((v) => v.id).toList());
  print(shadowedParameter(8).map((v) => v.id).toList());
  print(alternateWrite(1, 2).map((v) => v.id).toList());
  print(unknownCounter(2, 1).map((v) => v.id).toList());
  print(dynamicOverflow(2, 1).map((v) => v.id).toList());
  print(effects);
}
