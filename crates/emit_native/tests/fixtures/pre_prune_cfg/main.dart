import 'dart:io';

class Dead {
  Dead() { print('dead constructor'); }
  void invoke() { print('dead selector'); }
}

void main() {
  print('before branch');
  if (Platform.isAndroid) {
    dynamic receiver = Dead();
    receiver.invoke();
  }
  if (Platform.isWindows) {
    print('windows branch');
  } else {
    print('other branch');
  }
  print('after branch');
}
