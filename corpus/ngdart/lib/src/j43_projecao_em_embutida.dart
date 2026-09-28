import 'package:ngdart/angular.dart';

/// O índice do `<ng-content>` é o ordinal no template inteiro, também
/// dentro de `*ngIf` (o `li-page-header` do limitless_ui).
@Component(
  selector: 'j43-projecao-em-embutida',
  templateUrl: 'j43_projecao_em_embutida.html',
  directives: [NgIf],
)
class J43ProjecaoEmEmbutida {
  bool a = true;
  bool b = true;
}
