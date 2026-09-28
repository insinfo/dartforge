// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j51_exports.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j51_exports.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'j51_rotas.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$J51Exports = const [];

class ViewJ51Exports0 extends import0.ComponentView<import1.J51Exports> {
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  final import2.TextBinding _textBinding_5 = import2.TextBinding();
  final import2.TextBinding _textBinding_7 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_2;
  late final import3.AnchorElement _el_2;
  late final import3.HtmlElement _el_4;
  late final import3.HtmlElement _el_6;
  static import4.ComponentStyles? _componentStyles;
  ViewJ51Exports0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j51-exports'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j51_exports.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    final _el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'h1');
    final _text_1 = import8.appendText(_el_0, import9.interpolate0(import10.J51Rotas.titulo));
    this._el_2 = import8.appendElement<import3.AnchorElement>(doc, parentRenderNode, 'a');
    this._el_2.append(this._textBinding_3.element);
    this._el_4 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_4.append(this._textBinding_5.element);
    this._el_6 = import8.appendSpan(doc, parentRenderNode);
    this._el_6.append(this._textBinding_7.element);
    this._el_2.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = import10.J51Rotas.inicio.url();
    if (import11.checkBinding(this._expr_0, currVal_0, 'J51Rotas.inicio.url()', 'package:corpus_ngdart/src/j51_exports.html')) {
      import8.setProperty(this._el_2, 'title', currVal_0) /* REF:package:corpus_ngdart/src/j51_exports.html:32:63 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_3.updateText(import9.interpolate0(import10.J51Rotas.agora)) /* REF:package:corpus_ngdart/src/j51_exports.html:98:116 */;
    if (firstCheck) {
      if ((import10.j51Versao != null)) {
        import8.setProperty(this._el_4, 'title', import10.j51Versao) /* REF:package:corpus_ngdart/src/j51_exports.html:124:143 */;
      }
    }
    this._textBinding_5.updateText(import9.interpolate0(import10.j51Formatar(import10.j51Versao))) /* REF:package:corpus_ngdart/src/j51_exports.html:144:170 */;
    final currVal_2 = (_ctx.modo == import10.J51Modo.escuro);
    if (import11.checkBinding(this._expr_2, currVal_2, 'modo == J51Modo.escuro', 'package:corpus_ngdart/src/j51_exports.html')) {
      import8.updateClassBinding(this._el_6, 'escuro', currVal_2) /* REF:package:corpus_ngdart/src/j51_exports.html:181:220 */;
      this._expr_2 = currVal_2;
    }
    this._textBinding_7.updateText(import9.interpolate0(import10.J51Rotas.inicio.valor)) /* REF:package:corpus_ngdart/src/j51_exports.html:221:246 */;
  }

  void _handleEvent_0($event) {
    import10.J51Rotas.registrar('a');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J51Exports, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J51ExportsNgFactory = ComponentFactory<import1.J51Exports>('j51-exports', viewFactory_J51ExportsHost0);
ComponentFactory<import1.J51Exports> get J51ExportsNgFactory {
  return _J51ExportsNgFactory;
}

ComponentFactory<import1.J51Exports> createJ51ExportsFactory() {
  return ComponentFactory('j51-exports', viewFactory_J51ExportsHost0);
}

final List<Object> styles$J51ExportsHost = const [];

class _ViewJ51ExportsHost0 extends import13.HostView<import1.J51Exports> {
  @override
  void build() {
    this.componentView = ViewJ51Exports0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J51Exports();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.J51Exports> viewFactory_J51ExportsHost0() {
  return _ViewJ51ExportsHost0();
}
