import 'environment_default.dart'
    if (dart.library.notreal == 'false') 'environment_wrong.dart'
    if (dart.library.html == 'false') 'environment_wrong.dart' as choice;
import 'environment_default.dart'
    if (dart.library.mirrors) 'environment_mirrors.dart' as reflection;

void main() {
  print('dart.library.io');
  print(const bool.hasEnvironment('dart.library.io'));
  print(const String.fromEnvironment('dart.library.io', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.library.io', defaultValue: true));
  print('dart.library.mirrors');
  print(const bool.hasEnvironment('dart.library.mirrors'));
  print(const String.fromEnvironment('dart.library.mirrors', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.library.mirrors', defaultValue: true));
  print('dart.library.html');
  print(const bool.hasEnvironment('dart.library.html'));
  print(const String.fromEnvironment('dart.library.html', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.library.html', defaultValue: true));
  print('dart.library.js_interop');
  print(const bool.hasEnvironment('dart.library.js_interop'));
  print(const String.fromEnvironment('dart.library.js_interop', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.library.js_interop', defaultValue: true));
  print('dart.library.notreal');
  print(const bool.hasEnvironment('dart.library.notreal'));
  print(const String.fromEnvironment('dart.library.notreal', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.library.notreal', defaultValue: true));
  print('dart.vm.product');
  print(const bool.hasEnvironment('dart.vm.product'));
  print(const String.fromEnvironment('dart.vm.product', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.vm.product', defaultValue: true));
  print('dart.vm.profile');
  print(const bool.hasEnvironment('dart.vm.profile'));
  print(const String.fromEnvironment('dart.vm.profile', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.vm.profile', defaultValue: true));
  print('dart.isVM');
  print(const bool.hasEnvironment('dart.isVM'));
  print(const String.fromEnvironment('dart.isVM', defaultValue: 'missing'));
  print(const bool.fromEnvironment('dart.isVM', defaultValue: true));
  print(choice.value);
  print(reflection.value);
}
