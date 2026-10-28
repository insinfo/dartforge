// `main(List<String> args)` receives an empty `List<String>`, as on the VM
// run without arguments and as `dart2js` passes it: never `undefined`.
void main(List<String> args) {
  print(args.length);
  print(args.isEmpty);
  print(args is List<String>);
  print('[${args.join(',')}]');
  for (final a in args) {
    print('argument $a');
  }
  print(args.map((a) => a.length).toList());
  print(args.contains('x'));
}
