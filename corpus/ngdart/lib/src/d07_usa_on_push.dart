import 'package:ngdart/angular.dart';

import 'd07_filho_on_push.dart';

/// Filho `onPush`: `changed` e `markAsCheckOnce`; atributos do elemento
/// (`class` por `updateChildClassNonHtml`); entrada imutável no
/// `if (firstCheck)` com o teste de nulo; `#ref` no filho lido pelo
/// `@ViewChild`, com o `ChangeDetectorRef` registrado.
@Component(
  selector: 'd07-usa-on-push',
  templateUrl: 'd07_usa_on_push.html',
  directives: [D07FilhoOnPush],
)
class D07UsaOnPush {
  final String? constante = 'k';

  @ViewChild('painel')
  D07FilhoOnPush? painel;
}
