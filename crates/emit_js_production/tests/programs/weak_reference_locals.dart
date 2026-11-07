@pragma('weak-tearoff-reference')
Function? weak(Function? value) => value;
int chainTarget() => 1;
int shadowA() => 2;
int shadowB() => 3;
int loopTarget() => 4;
int unusedInitTarget() => 5;
int multiTarget() => 6;
int liveTarget() => 7;
int staticConstTarget() => 8;
int conditionalTarget() => 9;
int otherTarget() => 10;
class AliasHolder { static const alias = staticConstTarget; }
void check(bool value, String message) { if (!value) throw StateError(message); }
void main() {
  const first = chainTarget, second = first;
  check(weak(second) == null, 'transitive local const aliases');
  const alias = shadowA;
  { const alias = shadowB; check(weak(alias) == null, 'inner shadow'); }
  check(weak(alias) == null, 'outer shadow');
  for (const loop = loopTarget;;) {
    check(weak(loop) == null, 'constant loop initializer');
    break;
  }
  const unused = unusedInitTarget;
  check(weak(unusedInitTarget) == null, 'unused constant initializer');
  const dead = multiTarget, live = liveTarget;
  check(weak(dead) == null, 'mixed constant list dead member');
  check(weak(live) != null && live() == 7, 'mixed constant list strong member');
  const liveAlias = live;
  check(weak(liveAlias) != null && liveAlias() == 7, 'strong transitive alias');
  check(weak(AliasHolder.alias) == null, 'static constant alias');
  const choose = true;
  const selected = choose ? conditionalTarget : otherTarget;
  check(weak(selected) == null, 'constant conditional');
  print('local weak contracts ok');
}
