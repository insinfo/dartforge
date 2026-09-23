import 'package:ngdart/angular.dart';

/// Ligações constantes: consomem índice de ligação sem campo, saem antes das
/// dinâmicas do mesmo elemento e dividem o `if (firstCheck)` com as do
/// elemento seguinte (`addStmtsIfFirstCheck`).
@Component(
  selector: 'c11-ligacoes-constantes',
  templateUrl: 'c11_ligacoes_constantes.html',
)
class C11LigacoesConstantes {
  bool escondido = false;
}