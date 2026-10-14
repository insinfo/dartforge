/// Differential null-predicate and eager Boolean evaluation fixture.
/// Equality overrides must not run for a null RHS. This measures semantics,
/// not elapsed performance; never-inline keeps predicate bodies inspectable.
int equalCalls = 0;
int eagerCalls = 0;
class EqualProbe {
  @override
  bool operator ==(Object other) { equalCalls++; return true; }
  @override
  int get hashCode => 17;
}
@pragma('vm:never-inline')
bool missing(Object? value) => value == null;
@pragma('vm:never-inline')
bool present(Object? value) => value != null;
@pragma('vm:never-inline')
bool touch(bool value) { eagerCalls++; return value; }
void main() {
  final object = EqualProbe();
  print('${missing(null)} ${missing(object)} ${present(null)} ${present(object)} equality=$equalCalls');
  print('${touch(false) | true} ${touch(true) & false} eager=$eagerCalls');
  print('${missing(0)} ${missing(false)} ${missing(-0.0)} ${missing(double.nan)} equality=$equalCalls');
}
