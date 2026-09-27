// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j20_atributo_sem_valor.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j20_atributo_sem_valor.dart' as import1;
import 'j19_alternar.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/devtools.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J20AtributoSemValor = const [];

class ViewJ20AtributoSemValor0 extends import0.ComponentView<import1.J20AtributoSemValor> {
  late final import2.J19Alternar _J19Alternar_0_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ20AtributoSemValor0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j20-atributo-sem-valor'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j20_atributo_sem_valor.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.AnchorElement>(doc, parentRenderNode, 'a');
    import8.setAttribute(_el_0, 'href', '#filtros');
    import8.setAttribute(_el_0, 'j19Alternar', '');
    this._J19Alternar_0_5 = import2.J19Alternar();
    if (import9.isDevToolsEnabled) {
      import9.Inspector.instance.registerDirective(_el_0, this._J19Alternar_0_5);
    }
    final _text_1 = import8.appendText(_el_0, 'filtros');
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import9.isDevToolsEnabled) {
        import9.Inspector.instance.recordInput(this._J19Alternar_0_5, 'j19Alternar', '');
      }
      this._J19Alternar_0_5.alvo = '' /* REF:package:corpus_ngdart/src/j20_atributo_sem_valor.html:22:33 */;
      if (import9.isDevToolsEnabled) {
        import9.Inspector.instance.recordInput(this._J19Alternar_0_5, 'j19Animar', false);
      }
      this._J19Alternar_0_5.j19Animar = false /* REF:package:corpus_ngdart/src/j20_atributo_sem_valor.html:37:56 */;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J20AtributoSemValor, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J20AtributoSemValorNgFactory = ComponentFactory<import1.J20AtributoSemValor>('j20-atributo-sem-valor', viewFactory_J20AtributoSemValorHost0);
ComponentFactory<import1.J20AtributoSemValor> get J20AtributoSemValorNgFactory {
  return _J20AtributoSemValorNgFactory;
}

ComponentFactory<import1.J20AtributoSemValor> createJ20AtributoSemValorFactory() {
  return ComponentFactory('j20-atributo-sem-valor', viewFactory_J20AtributoSemValorHost0);
}

final List<Object> styles$J20AtributoSemValorHost = const [];

class _ViewJ20AtributoSemValorHost0 extends import11.HostView<import1.J20AtributoSemValor> {
  @override
  void build() {
    this.componentView = ViewJ20AtributoSemValor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J20AtributoSemValor();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J20AtributoSemValor> viewFactory_J20AtributoSemValorHost0() {
  return _ViewJ20AtributoSemValorHost0();
}
