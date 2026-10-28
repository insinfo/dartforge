// A `main` with two positional parameters receives the empty argument list
// and `null`, as on the VM and in `dart2js`.
void main(List<String> args, Object? message) {
  print('${args.length} ${args.isEmpty} $message ${message == null}');
}
