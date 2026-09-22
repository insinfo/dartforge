// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a16_entrada_e_saida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a16_entrada_e_saida.dart' as import1;
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

final List<Object> styles$A16EntradaESaida = const [];

class ViewA16EntradaESaida0 extends import0.ComponentView<import1.A16EntradaESaida> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA16EntradaESaida0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a16-entrada-e-saida'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a16_entrada_e_saida.dart' : null);
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
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.titulo)) /* REF:package:corpus_ngdart/src/a16_entrada_e_saida.html:5:15 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A16EntradaESaida, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A16EntradaESaidaNgFactory = ComponentFactory<import1.A16EntradaESaida>('a16-entrada-e-saida', viewFactory_A16EntradaESaidaHost0);
ComponentFactory<import1.A16EntradaESaida> get A16EntradaESaidaNgFactory {
  return _A16EntradaESaidaNgFactory;
}

ComponentFactory<import1.A16EntradaESaida> createA16EntradaESaidaFactory() {
  return ComponentFactory('a16-entrada-e-saida', viewFactory_A16EntradaESaidaHost0);
}

final List<Object> styles$A16EntradaESaidaHost = const [];

class _ViewA16EntradaESaidaHost0 extends import11.HostView<import1.A16EntradaESaida> {
  @override
  void build() {
    this.componentView = ViewA16EntradaESaida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A16EntradaESaida();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A16EntradaESaida> viewFactory_A16EntradaESaidaHost0() {
  return _ViewA16EntradaESaidaHost0();
}
