import 'package:ngdart/angular.dart';

import 'j19_alternar.dart';

/// Atributo sem valor que casa a entrada de uma diretiva, seguido de
/// quebra de linha: o `REF` cobre só o nome.
@Component(
  selector: 'j20-atributo-sem-valor',
  templateUrl: 'j20_atributo_sem_valor.html',
  directives: [J19Alternar],
)
class J20AtributoSemValor {}
