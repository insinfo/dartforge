import 'package:ngdart/angular.dart';

import 'h01_item.dart';

/// Componente de seletor composto que consulta o conteúdo projetado, como o
/// page header do new_sali: `@ContentChildren` com resultado, com e sem
/// `descendants`.
@Component(
  selector: 'h01-cabecalho,h01-cab',
  templateUrl: 'h01_cabecalho.html',
)
class H01Cabecalho {
  @Input()
  String titulo = '';

  @Input()
  bool mostrar = false;

  @ContentChildren(H01ItemDirective)
  List<H01ItemDirective> itens = <H01ItemDirective>[];

  @ContentChildren(H01AcoesDirective, descendants: false)
  List<H01AcoesDirective> acoes = <H01AcoesDirective>[];
}
