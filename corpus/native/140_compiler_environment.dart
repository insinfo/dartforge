// The reserved native definition keeps official VM fallbacks usable.
import '140_compiler_environment/definitions.dart' as environment;

void check(bool condition) {
  if (!condition) throw StateError('Compilation environment mismatch');
}

void main() {
  const key = 'dartforge.' + 'compiler';
  const detected = bool.fromEnvironment(key);
  const interpolated = bool.fromEnvironment('dartforge.${'compiler'}');
  const conditional = bool.fromEnvironment(detected ? key : 'dartforge.undefined');
  const present = bool.hasEnvironment('dartforge.\u0063ompiler');
  const name = String.fromEnvironment(key);
  const flagList = [environment.compilerFlag];
  const nameList = [environment.compilerAlias];
  const integerDefault = int.fromEnvironment(key, defaultValue: 23);
  check(detected == present);
  check(detected == interpolated);
  check(detected == conditional);
  check(detected == environment.compilerFlag);
  check(name == (detected ? 'true' : ''));
  check(integerDefault == 23);
  check(identical(flagList, detected ? const [true] : const [false]));
  check(identical(nameList, detected ? const ['true'] : const ['']));
  check(const bool.fromEnvironment('dartforge.undefined', defaultValue: true));
  check(!const bool.hasEnvironment('dartforge.undefined'));
  check(const String.fromEnvironment('dartforge.undefined', defaultValue: 'fallback') == 'fallback');
  print('Compilation environment validated');
}
