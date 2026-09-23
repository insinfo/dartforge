// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a24_interp_ternario_binario.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a24_interp_ternario_binario.dart' as import1;
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

final List<Object> styles$A24InterpTernarioBinario = const [];

class ViewA24InterpTernarioBinario0 extends import0.ComponentView<import1.A24InterpTernarioBinario> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  final import2.TextBinding _textBinding_5 = import2.TextBinding();
  final import2.TextBinding _textBinding_7 = import2.TextBinding();
  final import2.TextBinding _textBinding_9 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA24InterpTernarioBinario0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a24-interp-ternario-binario'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a24_interp_ternario_binario.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    final _el_2 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_2.append(this._textBinding_3.element);
    final _el_4 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_4.append(this._textBinding_5.element);
    final _el_6 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_6.append(this._textBinding_7.element);
    final _el_8 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_8.append(this._textBinding_9.element);
    final _el_10 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_11 = import8.appendText(_el_10, '1');
    final _el_12 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_13 = import8.appendText(_el_12, 'lit');
    final _el_14 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_15 = import8.appendText(_el_14, 'true');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolate0((_ctx.ativo ? 'sim' : 'não'))) /* REF:package:corpus_ngdart/src/a24_interp_ternario_binario.html:3:28 */;
    this._textBinding_3.updateText(import9.interpolateString0((_ctx.a + _ctx.b))) /* REF:package:corpus_ngdart/src/a24_interp_ternario_binario.html:35:44 */;
    this._textBinding_5.updateText(import9.interpolate0((_ctx.n + 1))) /* REF:package:corpus_ngdart/src/a24_interp_ternario_binario.html:51:60 */;
    this._textBinding_7.updateText(import9.interpolate0((!_ctx.ativo))) /* REF:package:corpus_ngdart/src/a24_interp_ternario_binario.html:67:77 */;
    this._textBinding_9.updateText(import9.interpolate0((_ctx.talvez ?? 'x'))) /* REF:package:corpus_ngdart/src/a24_interp_ternario_binario.html:84:101 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A24InterpTernarioBinario, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A24InterpTernarioBinarioNgFactory = ComponentFactory<import1.A24InterpTernarioBinario>('a24-interp-ternario-binario', viewFactory_A24InterpTernarioBinarioHost0);
ComponentFactory<import1.A24InterpTernarioBinario> get A24InterpTernarioBinarioNgFactory {
  return _A24InterpTernarioBinarioNgFactory;
}

ComponentFactory<import1.A24InterpTernarioBinario> createA24InterpTernarioBinarioFactory() {
  return ComponentFactory('a24-interp-ternario-binario', viewFactory_A24InterpTernarioBinarioHost0);
}

final List<Object> styles$A24InterpTernarioBinarioHost = const [];

class _ViewA24InterpTernarioBinarioHost0 extends import11.HostView<import1.A24InterpTernarioBinario> {
  @override
  void build() {
    this.componentView = ViewA24InterpTernarioBinario0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A24InterpTernarioBinario();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A24InterpTernarioBinario> viewFactory_A24InterpTernarioBinarioHost0() {
  return _ViewA24InterpTernarioBinarioHost0();
}
