import 'dart:async';

import 'package:ngdart/angular.dart';

/// Filho com dois `@Output`, um com apelido.
@Component(
  selector: 'd06-filho-saida',
  templateUrl: 'd06_filho_saida.html',
)
class D06FilhoSaida {
  final _salvo = StreamController<String>();
  final _fechou = StreamController<void>();

  @Output()
  Stream<String> get salvo => _salvo.stream;

  @Output('fechar')
  Stream<void> get fechou => _fechou.stream;
}
