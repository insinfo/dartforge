import 'dart:html';

import 'package:ngdart/angular.dart';

/// A ordem dos campos `_viewQuery_*_isDirty` é a do primeiro resultado
/// dinâmico no template (pré-ordem), não a das consultas (o `li-select` do
/// limitless_ui); e `[attr.class]`/`[className]` são o `updateChildClass`,
/// como `[class]` (o `treeview-select`).
@Component(
  selector: 'j39-ordem-dos-sujos',
  templateUrl: 'j39_ordem_dos_sujos.html',
  styles: ['.x { color: red; }'],
  directives: [NgIf],
)
class J39OrdemDosSujos {
  bool a = true;
  bool b = true;
  String cls = 'c';

  @ViewChild('campo')
  InputElement? campo;

  @ViewChild('botao')
  Element? botao;
}
