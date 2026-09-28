import 'package:ngdart/angular.dart';

/// `preserveWhitespace: true` (o `material_popup` e os tooltips do
/// ngcomponents): nenhum texto some nem é aparado, só o `&ngsp;` vira
/// espaço; as pontas das interpolações não são comprimidas.
@Component(
  selector: 'j115-preserva',
  template: '''
<div class="a">
  <span>{{ nome }}</span>
  &ngsp;<b *ngIf="mostra">x</b>
</div>
<p title="
  {{ nome }}
">  fim  </p>
''',
  directives: [NgIf],
  preserveWhitespace: true,
)
class J115Preserva {
  String nome = 'n';
  bool mostra = true;
}

@Component(
  selector: 'j115-minimiza',
  template: '''
<div>
  <span>{{ nome }}</span>
</div>
''',
  preserveWhitespace: false,
)
class J115Minimiza {
  String nome = 'n';
}
