// Regressão: o `main(List<String> args)` recebe a linha de comando
// (uma lista fixa de `String`, como a do embedder da VM).
void main(List<String> args) {
  print(args);
  print(args is List<String>);
  try {
    args.add('x');
  } on UnsupportedError {
    print('fixa');
  }
}
