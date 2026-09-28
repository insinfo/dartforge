import 'dart:html';

import 'package:ngdart/angular.dart';

@Component(
  selector: 'j77-modal',
  template: '<div class="corpo"><ng-content></ng-content></div>',
)
class J77Modal {}

@Component(
  selector: 'j77-tabela',
  template: '<i></i>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J77Tabela {}

/// `@ViewChild` de um filho dentro de `*ngIf` no conteúdo projetado de
/// outro filho (o `#datatable` dentro do `li-modal` do
/// `li-datatable-select`).
@Component(
  selector: 'j77-consulta-em-projetado',
  templateUrl: 'j77_consulta_em_projetado.html',
  directives: [J77Modal, J77Tabela, NgIf],
)
class J77ConsultaEmProjetado {
  bool mostrar = true;

  @ViewChild('modal')
  J77Modal? modal;

  @ViewChild('tabela')
  J77Tabela? tabela;

  @ViewChild('rodape')
  Element? rodape;
}
