import 'package:ngdart/angular.dart';

// Comentário com acentuação (ção, é) antes dos templates: as posições do
// `REF` contam em unidades UTF-16.

/// Sonda: três componentes no arquivo, o que usa os outros primeiro, com
/// ligações em template escrito na anotação e em `templateUrl`.
@Component(
  selector: 'i79-varios-componentes',
  template:
      '<i79-folha [valor]="n" (click)="n = n + 1"></i79-folha><i79-outra></i79-outra>',
  directives: [I79Folha, I79Outra],
)
class I79VariosComponentes {
  int n = 1;
}

@Component(selector: 'i79-folha', template: "<span>{{valor}} é {{valor}}</span>")
class I79Folha {
  @Input()
  int valor = 0;
}

@Component(
  selector: 'i79-outra',
  templateUrl: 'i79_varios_componentes.html',
  directives: [NgIf],
)
class I79Outra {
  bool ver = true;
}
