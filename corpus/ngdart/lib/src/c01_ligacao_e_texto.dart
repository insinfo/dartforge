import 'package:ngdart/angular.dart';

/// Ligação de propriedade e interpolação no mesmo template: a ordem dos
/// campos da visão (`_expr_`, `_textBinding_`, `_el_`).
@Component(
  selector: 'c01-ligacao-e-texto',
  templateUrl: 'c01_ligacao_e_texto.html',
)
class C01LigacaoETexto {
  bool escondido = false;
  String texto = 'x';
}
