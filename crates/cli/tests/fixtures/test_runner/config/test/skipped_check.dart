@Skip('the whole suite is skipped')
library;

import 'package:test/test.dart';

void main() {
  test('never runs', () => fail('skipped suite ran'));
}
