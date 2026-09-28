import 'package:ngdart/angular.dart';

/// Filho sem `<ng-content>`.
@Component(
  selector: 'j37-sem-slot',
  template: '<b>fixo</b>',
)
class J37SemSlot {
  @Input()
  String? titulo;
}

/// Filho que projeta tudo.
@Component(
  selector: 'j37-com-slot',
  template: '<div><ng-content></ng-content></div>',
)
class J37ComSlot {}

/// Filho que só projeta `b`.
@Component(
  selector: 'j37-so-b',
  template: '<p><ng-content select="b"></ng-content></p>',
)
class J37SoB {}

/// Conteúdo passado a um filho que não projeta (o `li-tab`/`li-card` do
/// limitless_ui com filhos que o componente ignora): o oficial cria os nós
/// e as ligações do conteúdo mesmo sem os anexar a nada.
@Component(
  selector: 'j37-filho-sem-projecao',
  templateUrl: 'j37_filho_sem_projecao.html',
  directives: [J37SemSlot, J37ComSlot, J37SoB, NgIf],
)
class J37FilhoSemProjecao {
  String nome = 'n';
}
