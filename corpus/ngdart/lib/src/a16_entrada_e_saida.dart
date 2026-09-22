import 'dart:async';

import 'package:ngdart/angular.dart';

/// `@Input`/`@Output` no próprio componente.
@Component(
  selector: 'a16-entrada-e-saida',
  templateUrl: 'a16_entrada_e_saida.html',
)
class A16EntradaESaida {
  @Input()
  String titulo = '';

  final StreamController<String> _mudou = StreamController<String>();

  @Output()
  Stream<String> get mudou => _mudou.stream;
}
