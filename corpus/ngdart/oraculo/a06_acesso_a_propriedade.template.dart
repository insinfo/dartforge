// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a06_acesso_a_propriedade.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a06_acesso_a_propriedade.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$A06AcessoAPropriedade = const [];

class ViewA06AcessoAPropriedade0 extends import0.ComponentView<import1.A06AcessoAPropriedade> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA06AcessoAPropriedade0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a06-acesso-a-propriedade'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a06_acesso_a_propriedade.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.pessoa.nome)) /* REF:package:corpus_ngdart/src/a06_acesso_a_propriedade.html:5:20 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A06AcessoAPropriedade, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A06AcessoAPropriedadeNgFactory = ComponentFactory<import1.A06AcessoAPropriedade>('a06-acesso-a-propriedade', viewFactory_A06AcessoAPropriedadeHost0);
ComponentFactory<import1.A06AcessoAPropriedade> get A06AcessoAPropriedadeNgFactory {
  return _A06AcessoAPropriedadeNgFactory;
}

ComponentFactory<import1.A06AcessoAPropriedade> createA06AcessoAPropriedadeFactory() {
  return ComponentFactory('a06-acesso-a-propriedade', viewFactory_A06AcessoAPropriedadeHost0);
}

final List<Object> styles$A06AcessoAPropriedadeHost = const [];

class _ViewA06AcessoAPropriedadeHost0 extends import11.HostView<import1.A06AcessoAPropriedade> {
  @override
  void build() {
    this.componentView = ViewA06AcessoAPropriedade0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A06AcessoAPropriedade();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A06AcessoAPropriedade> viewFactory_A06AcessoAPropriedadeHost0() {
  return _ViewA06AcessoAPropriedadeHost0();
}
