import 'unrelated_length.dart' show Hot, Cold;

void main(List<String> arguments) {
  final hot = Hot();
  final cold = Cold();
  final dynamic receiver = arguments.contains('cold') ? cold : hot;
  print(receiver.length());
  print(cold.tag);
}
