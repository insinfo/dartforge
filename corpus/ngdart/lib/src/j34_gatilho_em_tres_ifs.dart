import 'dart:html';

import 'package:ngdart/angular.dart';

/// O mesmo `#ref` em três `*ngIf` e um `@ViewChild` dele (o
/// `triggerElement` do `li-date-picker` do limitless_ui): a consulta junta
/// os resultados das três visões e fica com o primeiro.
@Component(
  selector: 'j34-gatilho-em-tres-ifs',
  templateUrl: 'j34_gatilho_em_tres_ifs.html',
  directives: [coreDirectives],
)
class J34GatilhoEmTresIfs {
  bool a = true;
  bool b = false;

  @ViewChild('gatilho')
  Element? gatilho;

  @ViewChildren('gatilho')
  List<Element>? todos;

  void abrir() {}
}
