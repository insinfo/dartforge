import 'package:ngdart/angular.dart';

/// O `T` do próprio componente genérico: em escopo em toda visão dele, entra
/// como está na substituição dos tipos herdados. `opcoes` vem de um mixin
/// `J127ComOpcoes<T>` (`J127Opcoes<T>`, e `grupos` dá `J127Grupo<T>`);
/// `filhos` vem da superclasse `J127No<T?>` (`J127Grupo<T?>`).
class J127Grupo<T> {
  List<T> itens = [];
  String nome = '';
}

class J127Opcoes<T> {
  List<J127Grupo<T>> get grupos => [];
}

mixin J127ComOpcoes<T> {
  J127Opcoes<T> get opcoes => J127Opcoes<T>();
}

class J127No<T> {
  Iterable<J127Grupo<T>> filhos(Object? x) => [];
}

@Component(
  selector: 'j127-folha',
  template: '',
)
class J127Folha {
  @Input()
  Object? valor;
}

@Component(
  selector: 'j127-arvore',
  template: '''<div *ngFor="let g of opcoes.grupos">{{g.nome}}
  <j127-folha *ngFor="let f of filhos(g)" [valor]="f"></j127-folha>
</div>
<p *ngFor="let s of filhos(1)">{{s.nome}}</p>
<div *ngFor="let b of brutos">
  <j127-folha *ngFor="let y of filhos(b)" [valor]="y"></j127-folha>
</div>''',
  directives: [NgFor, J127Folha],
)
class J127Arvore<T> extends J127No<T?> with J127ComOpcoes<T> {
  List<dynamic> brutos = [];
}
