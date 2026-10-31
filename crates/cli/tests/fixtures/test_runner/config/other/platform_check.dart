@OnPlatform({'mac-os': Skip('skipped on macOS'), 'linux || windows': Timeout.factor(2)})
@Retry(2)
import 'package:test/test.dart';

void main() {
  test('platform dependent', () {});
  test('vm only test', () {}, testOn: 'vm');
  test('browser only test', () {}, testOn: 'browser');
}
