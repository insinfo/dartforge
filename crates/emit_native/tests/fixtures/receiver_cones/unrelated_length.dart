class Hot {
  int length() => 7;
}

class Cold {
  String get tag => 'cold';

  int length() {
    print('UNREACHABLE_COLD_LENGTH_BODY');
    return 99;
  }
}

void main() {
  final hot = Hot();
  final cold = Cold();
  print(hot.length());
  print(cold.tag);
}
