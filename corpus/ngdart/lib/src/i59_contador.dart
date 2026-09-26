import 'dart:async';

import 'package:ngdart/angular.dart';

/// Sonda: filho com `valor`/`valorChange`.
@Component(
  selector: 'i59-contador',
  templateUrl: 'i59_contador.html',
  directives: [],
)
class I59Contador {
  @Input()
  int valor = 0;
  final _c = StreamController<int>();
  @Output()
  Stream<int> get valorChange => _c.stream;
}
