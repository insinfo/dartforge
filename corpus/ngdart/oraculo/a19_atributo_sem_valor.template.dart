// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a19_atributo_sem_valor.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a19_atributo_sem_valor.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$A19AtributoSemValor = const [];

class ViewA19AtributoSemValor0 extends import0.ComponentView<import1.A19AtributoSemValor> {
  static import2.ComponentStyles? _componentStyles;
  ViewA19AtributoSemValor0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('a19-atributo-sem-valor'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/a19_atributo_sem_valor.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.InputElement>(doc, parentRenderNode, 'input');
    import7.setAttribute(_el_0, 'disabled', '');
    import7.setAttribute(_el_0, 'type', 'text');
    import7.setAttribute(_el_0, 'value', '');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$A19AtributoSemValor, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A19AtributoSemValorNgFactory = ComponentFactory<import1.A19AtributoSemValor>('a19-atributo-sem-valor', viewFactory_A19AtributoSemValorHost0);
ComponentFactory<import1.A19AtributoSemValor> get A19AtributoSemValorNgFactory {
  return _A19AtributoSemValorNgFactory;
}

ComponentFactory<import1.A19AtributoSemValor> createA19AtributoSemValorFactory() {
  return ComponentFactory('a19-atributo-sem-valor', viewFactory_A19AtributoSemValorHost0);
}

final List<Object> styles$A19AtributoSemValorHost = const [];

class _ViewA19AtributoSemValorHost0 extends import9.HostView<import1.A19AtributoSemValor> {
  @override
  void build() {
    this.componentView = ViewA19AtributoSemValor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A19AtributoSemValor();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.A19AtributoSemValor> viewFactory_A19AtributoSemValorHost0() {
  return _ViewA19AtributoSemValorHost0();
}
