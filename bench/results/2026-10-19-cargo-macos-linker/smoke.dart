void main(List<String> args) {
  int Function(int) multiply = (value) => value * 6;
  print('args:${args.join(',')}');
  print(multiply(7));
}
