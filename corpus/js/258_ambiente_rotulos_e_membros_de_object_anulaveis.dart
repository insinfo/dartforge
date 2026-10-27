// bool/String.fromEnvironment('dart.library.x') agree with the platform's
// libraries. A labeled declaration stays in its scope. A switch with
// continue to a case label keeps grouped empty cases sharing the next body.
// toString, hashCode and runtimeType of a nullable user type handle null.
enum Taste { sweet, sour, salty, umami }

int value(Taste taste, bool jump) {
  switch (taste) {
    case Taste.sweet:
    case Taste.sour:
      return 1;
    case Taste.salty:
      if (jump) continue later;
      return 2;
    later:
    case Taste.umami:
      return 4;
  }
}

class A {}

A? none() => null;

void main() {
  const core = bool.fromEnvironment('dart.library.core');
  const coreText = String.fromEnvironment('dart.library.core');
  const missing = bool.fromEnvironment('dart.library.nonexistent');
  print([core, coreText, missing]);
  label:
  var x = 1, y = 2;
  x += y;
  print(x);
  print([for (var t in Taste.values) value(t, true)]);
  print([for (var t in Taste.values) value(t, false)]);
  A? a = none();
  print(a.toString());
  print(a.hashCode == null.hashCode);
  print(a.runtimeType);
  var tearOff = a.toString;
  print(tearOff());
}
