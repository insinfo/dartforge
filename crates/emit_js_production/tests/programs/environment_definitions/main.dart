import 'fallback.dart' if (feature) 'enabled.dart' as flag;
import 'fallback.dart' if (flavor == 'yes') 'enabled.dart' as flavor;

const prefix = 'fea';
const key = '${prefix}ture';
const fallback = 'alias default';
class Keys {
  static const text = 'message';
}

dynamic observed(String label, dynamic value) {
  print(label);
  return value;
}

void main() {
  const local = 'message';
  print(const String.fromEnvironment(Keys.text, defaultValue: fallback));
  print(const String.fromEnvironment(local, defaultValue: fallback));
  print(const bool.fromEnvironment(key, defaultValue: true));
  print(const bool.hasEnvironment(key));
  print(const bool.hasEnvironment('empty'));
  print(const String.fromEnvironment('empty', defaultValue: fallback));
  print(const String.fromEnvironment('missing', defaultValue: fallback));
  print(const bool.fromEnvironment('invalidBool', defaultValue: true));
  print(const int.fromEnvironment('number', defaultValue: 17));
  print(const int.fromEnvironment('hex', defaultValue: 17));
  print(const int.fromEnvironment('invalidInt', defaultValue: 17));
  print(const bool.hasEnvironment('dart.vm.product'));
  print(const String.fromEnvironment('dart.vm.product', defaultValue: fallback));
  print(const bool.hasEnvironment('dart.library.missing'));
  print(const bool.hasEnvironment('dart.library.js'));
  print(const String.fromEnvironment('dart.library.js', defaultValue: fallback));
  print(const String.fromEnvironment('\uD800', defaultValue: fallback));
  print(const bool.hasEnvironment('\uD800'));
  print(const [bool.fromEnvironment(key), String.fromEnvironment('message')]);
  print(flag.choice);
  print(flavor.choice);
  try {
    String.fromEnvironment(
      defaultValue: observed('default first', 'fallback'),
      observed('name second', 'message'),
    );
  } catch (error) {
    print(error is UnsupportedError);
  }
  try {
    bool.hasEnvironment(observed('presence argument', 'message'));
  } catch (error) {
    print(error is UnsupportedError);
  }
  try {
    int.fromEnvironment(observed('integer argument', 'number'),
        defaultValue: observed('integer default', 0));
  } catch (error) {
    print(error is UnsupportedError);
  }
  try {
    bool.fromEnvironment(observed('invalid name', 42),
        defaultValue: observed('bool default', false));
  } catch (error) {
    print(error is TypeError);
  }
}
