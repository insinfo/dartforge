import 'package:ngdart/angular.dart';

/// Regras da seção 03 da especificação: o nome de `exports:` é resolvido no
/// parse, antes do local de template de mesmo nome; `StaticRead` é sempre
/// imutável (a anotação só aceita constantes, então isso vale para `const`,
/// classe e função); `Classe.metodo` é
/// imutável ("methods are immutable"); `-x` é `(0 - x)`.
const j112Contador = 3;

const j112Rotulo = 'r';

class J112Util {
  static String formatar(Object o) => '$o';
  static var mutavel = 1;
}

@Component(
  selector: 'j112-export-e-imutabilidade',
  template: '''
<p>{{ j112Contador }} {{ j112Rotulo }}</p>
<i [title]="J112Util.formatar"></i>
<b [title]="J112Util.mutavel"></b>
<div *ngFor="let j112Contador of itens">{{ j112Contador }}</div>
<span [title]="-n">{{ -2 }}</span>''',
  directives: [NgFor],
  exports: [j112Contador, j112Rotulo, J112Util],
)
class J112ExportEImutabilidade {
  final List<String> itens = ['a'];
  int n = 1;
}
