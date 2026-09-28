import 'package:ngdart/angular.dart';

/// Os campos dos nós saem na ordem em que o `NodeReferenceStorageVisitor`
/// os promove: a primeira leitura fora do `build()`. As declarações dos
/// locais (`final local_x = this._el_n;`) ficam no topo do
/// `detectChangesInternal`, então o nó de um `#ref` lido na detecção vem
/// antes dos nós das ligações (o `_el_8` do `material_stepper`).
@Component(
  selector: 'j122-usa',
  template: '''<div [attr.a]="v"></div>
<p [title]="depois.id"></p>
<span #depois [attr.b]="v"></span>
<section *ngIf="mostra"><i [attr.c]="v"></i><b [title]="la.id"></b><em #la></em></section>''',
  directives: [NgIf],
)
class J122Usa {
  String v = 'x';
  bool mostra = true;
}
