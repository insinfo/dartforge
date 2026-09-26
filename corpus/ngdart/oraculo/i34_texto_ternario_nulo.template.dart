// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i34_texto_ternario_nulo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i34_texto_ternario_nulo.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I34TextoTernarioNulo = const [];

class ViewI34TextoTernarioNulo0 extends import0.ComponentView<import1.I34TextoTernarioNulo> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  Object? _expr_0;
  late final import3.HtmlElement _el_4;
  static import4.ComponentStyles? _componentStyles;
  ViewI34TextoTernarioNulo0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('i34-texto-ternario-nulo'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i34_texto_ternario_nulo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    final _el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    final _text_2 = import8.appendText(_el_0, ' ');
    _el_0.append(this._textBinding_3.element);
    this._el_4 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolate0((_ctx.nome ?? ''))) /* REF:package:corpus_ngdart/src/i34_texto_ternario_nulo.html:3:17 */;
    this._textBinding_3.updateText(import9.interpolate0((_ctx.ativo ? _ctx.nome : '-'))) /* REF:package:corpus_ngdart/src/i34_texto_ternario_nulo.html:18:40 */;
    final currVal_0 = _ctx.lista.isEmpty;
    if (import10.checkBinding(this._expr_0, currVal_0, 'lista.isEmpty', 'package:corpus_ngdart/src/i34_texto_ternario_nulo.html')) {
      import8.setProperty(this._el_4, 'hidden', currVal_0) /* REF:package:corpus_ngdart/src/i34_texto_ternario_nulo.html:47:71 */;
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I34TextoTernarioNulo, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I34TextoTernarioNuloNgFactory = ComponentFactory<import1.I34TextoTernarioNulo>('i34-texto-ternario-nulo', viewFactory_I34TextoTernarioNuloHost0);
ComponentFactory<import1.I34TextoTernarioNulo> get I34TextoTernarioNuloNgFactory {
  return _I34TextoTernarioNuloNgFactory;
}

ComponentFactory<import1.I34TextoTernarioNulo> createI34TextoTernarioNuloFactory() {
  return ComponentFactory('i34-texto-ternario-nulo', viewFactory_I34TextoTernarioNuloHost0);
}

final List<Object> styles$I34TextoTernarioNuloHost = const [];

class _ViewI34TextoTernarioNuloHost0 extends import12.HostView<import1.I34TextoTernarioNulo> {
  @override
  void build() {
    this.componentView = ViewI34TextoTernarioNulo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I34TextoTernarioNulo();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I34TextoTernarioNulo> viewFactory_I34TextoTernarioNuloHost0() {
  return _ViewI34TextoTernarioNuloHost0();
}
