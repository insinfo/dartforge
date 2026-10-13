import 'dart:async';

void a = (throw 0);

FutureOr<Object?> Function() f = () async {
  return a;
};
