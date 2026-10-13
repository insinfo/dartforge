// Regression: `main(List<String> args)` receives the command line
// (a fixed-length list of `String`, like the VM embedder's).
void main(List<String> args) {
  print(args);
  print(args is List<String>);
  try {
    args.add('x');
  } on UnsupportedError {
    print('fixa');
  }
}
