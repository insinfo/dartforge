@pragma('weak-tearoff-reference')
Function? weakRef(Function? target) => target;

int usedByCall() => 10;
int usedByTearoff() => 20;
int unusedByCall() => 30;
int unusedByTearoff() => 40;

class Owner {
  int liveCall() => usedByCall() + 1;
  Function liveTearoff() => usedByTearoff;
  int deadCall() => unusedByCall() + 1;
  Function deadTearoff() => unusedByTearoff;
}

void check(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void main() {
  final first = weakRef(usedByCall);
  final second = weakRef(usedByTearoff);
  check(first != null && first() == 10, 'live direct target');
  check(second != null && second() == 20, 'live tearoff target');
  check(weakRef(unusedByCall) == null, 'weak reference kept unused direct target');
  check(weakRef(unusedByTearoff) == null, 'weak reference kept unused tearoff target');
  final owner = Owner();
  check(owner.liveCall() == 11, 'strong direct use');
  check(owner.liveTearoff()() == 20, 'strong tearoff use');
  print('weak tearoff contracts ok');
}
