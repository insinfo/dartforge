import 'package:ngdart/angular.dart';

/// `_attrToPropMap` do esquema: `readonly` → `readOnly`, `tabindex` →
/// `tabIndex` (e `innerHtml` → `innerHTML`); `[x]` e `x="{{..}}"` viram
/// `setProperty` com o nome mapeado. `style="{{..}}"` é a propriedade
/// `style`, saneada como estilo.
@Component(
  selector: 'j114-propriedade-do-esquema',
  template: '''
<input [tabindex]="n" [readonly]="r">
<div tabindex="{{n}}" style="{{estilo}}"></div>
<span style.width="calc({{100-n}}%)"></span>''',
)
class J114PropriedadeDoEsquema {
  int n = 1;
  bool r = false;
  String estilo = 'color: red';
}
