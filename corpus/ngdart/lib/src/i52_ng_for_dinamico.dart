import 'package:ngdart/angular.dart';

class Pessoa {
  String nome = 'n';
  int idade = 1;
}

/// Sonda: local `dynamic` de `*ngFor` (coleção com `??`) lido na visão aninhada.
@Component(
  selector: 'i52-ng-for-dinamico',
  templateUrl: 'i52_ng_for_dinamico.html',
  directives: [coreDirectives],
)
class I52NgForDinamico {
  List<Pessoa>? grupos;
  List<Pessoa> vazio = [];
  List<String> itens = ['a'];
  void usar(Object? g, String x) {}
}
