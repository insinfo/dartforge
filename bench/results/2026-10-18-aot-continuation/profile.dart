
int evaluated = 0;
bool note(bool value) { evaluated++; return value; }
class Box { final int n; Box(this.n) : assert(note(n > 0), 'positive'); }
void main() {
  try {
    assert(note(false), 'message ${note(true)}');
    print('assert skipped');
  } on AssertionError catch (e) {
    print('AssertionError: ${e.message}');
  }
  print('evaluated: $evaluated');
  try { Box(0); print('box built'); } on AssertionError { print('box refused'); }
  print('product: ${const bool.fromEnvironment("dart.vm.product")}');
  print('has product: ${const bool.hasEnvironment("dart.vm.product")}');
  print('profile: ${const bool.fromEnvironment("dart.vm.profile", defaultValue: true)}');
  print('text: ${const String.fromEnvironment("dart.vm.product", defaultValue: "absent")}');
}
