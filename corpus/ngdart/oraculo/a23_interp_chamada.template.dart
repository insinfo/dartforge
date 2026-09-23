// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a23_interp_chamada.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a23_interp_chamada.dart' as import1;
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

final List<Object> styles$A23InterpChamada = const [];

class ViewA23InterpChamada0 extends import0.ComponentView<import1.A23InterpChamada> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  final import2.TextBinding _textBinding_5 = import2.TextBinding();
  final import2.TextBinding _textBinding_7 = import2.TextBinding();
  final import2.TextBinding _textBinding_9 = import2.TextBinding();
  final import2.TextBinding _textBinding_11 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewA23InterpChamada0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('a23-interp-chamada'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a23_interp_chamada.dart' : null);
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
    _el_10.append(this._textBinding_11.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.titulo())) /* REF:package:corpus_ngdart/src/a23_interp_chamada.html:3:15 */;
    this._textBinding_3.updateTextWithPrimitive(_ctx.contar()) /* REF:package:corpus_ngdart/src/a23_interp_chamada.html:22:34 */;
    this._textBinding_5.updateText(import9.interpolateString0(_ctx.conta.descricao())) /* REF:package:corpus_ngdart/src/a23_interp_chamada.html:41:62 */;
    this._textBinding_7.updateTextWithPrimitive(_ctx.conta.total()) /* REF:package:corpus_ngdart/src/a23_interp_chamada.html:69:86 */;
    this._textBinding_9.updateText(import9.interpolate0(_ctx.nomes[0])) /* REF:package:corpus_ngdart/src/a23_interp_chamada.html:93:105 */;
    this._textBinding_11.updateText(import9.interpolate0((_ctx.talvez!))) /* REF:package:corpus_ngdart/src/a23_interp_chamada.html:112:123 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A23InterpChamada, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A23InterpChamadaNgFactory = ComponentFactory<import1.A23InterpChamada>('a23-interp-chamada', viewFactory_A23InterpChamadaHost0);
ComponentFactory<import1.A23InterpChamada> get A23InterpChamadaNgFactory {
  return _A23InterpChamadaNgFactory;
}

ComponentFactory<import1.A23InterpChamada> createA23InterpChamadaFactory() {
  return ComponentFactory('a23-interp-chamada', viewFactory_A23InterpChamadaHost0);
}

final List<Object> styles$A23InterpChamadaHost = const [];

class _ViewA23InterpChamadaHost0 extends import11.HostView<import1.A23InterpChamada> {
  @override
  void build() {
    this.componentView = ViewA23InterpChamada0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A23InterpChamada();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.A23InterpChamada> viewFactory_A23InterpChamadaHost0() {
  return _ViewA23InterpChamadaHost0();
}
