import 'dart:_foreign_helper' show JS;

class Item {
  Item() {
    JS('void', r'globalThis.__freshLeaked && (globalThis.__freshLeaked.fixed$length = Array)');
  }
}
List<Item> fresh(int n) {
  final list = <Item>[];
  for (var i = 0; i < n; i++) { list.add(Item()); }
  return list;
}
void main() {
  JS('void', r'''(() => {
    const key = Object.getOwnPropertySymbols(Array.prototype).find(s => s.description === 'dartx.add');
    const original = Object.getOwnPropertyDescriptor(Array.prototype, key);
    globalThis.__freshRestore = () => Object.defineProperty(Array.prototype, key, original);
    Object.defineProperty(Array.prototype, key, {configurable:true, get() {
      globalThis.__freshLeaked = this;
      return original.value;
    }});
  })()''');
  try { fresh(1); } catch (error) { print(error is UnsupportedError); }
  JS('void', r'globalThis.__freshRestore()');
}
