import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';

class Pessoa {
  String nome = 'n';
  int idade = 1;
}

/// Sonda: `#ref` citado só como membro, lido em `*` antes de declarado, de filho em visão aninhada e de duas visões acima.
@Component(
  selector: 'i45-ref-formas',
  templateUrl: 'i45_ref_formas.html',
  directives: [coreDirectives, A02TextoEstatico],
)
class I45RefFormas {
  Pessoa p = Pessoa();
  List<String> itens = ['a'];
  void usar(Object? a, Object? b, Object? c) {}
}
