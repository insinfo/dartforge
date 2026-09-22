// Convertido de tests/conformance/cases/sealed_patterns.dart (fixture antigo do corpus de conformidade).
sealed class Login {}
int mark() { print('constructed'); return 1; }
class Authenticated extends Login { int marker = mark(); }
sealed class Waiting extends Login {}
class Loading extends Waiting {}
class Queued extends Waiting {}
class Open extends Login {}
class OpenChild extends Open {}
class Implemented implements Login {}
String describe(Login? value) => switch (value) {
  null => 'null',
  Authenticated() when false => 'unreachable',
  Authenticated() => 'authenticated',
  Waiting() => 'waiting',
  Open() => 'open',
  Implemented() => 'implemented',
};
void main() {
  print(describe(null));
  print(describe(Authenticated()));
  print(describe(Loading()));
  print(describe(Queued()));
  print(describe(Open()));
  print(describe(OpenChild()));
  print(describe(Implemented()));
}
