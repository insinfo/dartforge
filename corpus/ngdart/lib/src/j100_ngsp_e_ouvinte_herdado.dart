import 'dart:html';

import 'package:ngdart/angular.dart';

/// `&ngsp;` depois de elemento de bloco (o `visitText` troca por espaço
/// depois de aparar) e `@HostListener` herdado da base (supertipos
/// primeiro; o mesmo evento fica com o do derivado, no lugar do primeiro).
class J100Base {
  @HostListener('keydown', [r'$event'])
  void tecla(KeyboardEvent e) {}

  @HostListener('blur')
  void saiu() {}

  @HostListener('focus')
  void entrouNaBase() {}

  String? descricao = 'x';
  bool mostra = true;
}

@Component(
  selector: 'j100-ngsp',
  template: '''
<span *ngIf="descricao != null" class="d">
  <div *ngIf="mostra" class="g">
  </div>
  &ngsp;{{descricao}}&ngsp;
  <ng-content></ng-content>
</span>
<p>
  &ngsp;fim&ngsp;
</p>''',
  directives: [NgIf],
)
class J100Ngsp extends J100Base {
  @HostListener('focus', [r'$event'])
  void entrou(FocusEvent e) {}

  @HostListener('mousedown')
  void apertou() {}
}
