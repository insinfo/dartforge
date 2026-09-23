import 'package:ngdart/angular.dart';

/// Eventos dentro de `*ngFor`: o tear-off lê `_ctx` no `build()` da visão
/// embutida; o handler complexo declara os locais que lê no topo do
/// `_handleEvent_N`, na ordem em que o conversor os pede.
@Component(
  selector: 'c13-evento-em-ng-for',
  templateUrl: 'c13_evento_em_ng_for.html',
  directives: [coreDirectives],
)
class C13EventoEmNgFor {
  List<String> itens = ['a', 'b'];
  void limpar() {}
  void escolher(String item, int i) {}
}
