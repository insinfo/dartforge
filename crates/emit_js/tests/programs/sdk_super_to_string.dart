import 'dart:js';
void main() {
  final object = JsObject.jsify({});
  object['toString'] = JsFunction.withThis((self) { throw StateError('foreign conversion failed'); });
  final text = object.toString();
  if (text.isEmpty) throw 'missing Object fallback';
  print('JS Object fallback ok');
}
