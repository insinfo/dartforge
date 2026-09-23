// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c12_handler_atribuicao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c12_handler_atribuicao.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$C12HandlerAtribuicao = const [];

class ViewC12HandlerAtribuicao0 extends import0.ComponentView<import1.C12HandlerAtribuicao> {
  static import2.ComponentStyles? _componentStyles;
  ViewC12HandlerAtribuicao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('c12-handler-atribuicao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/c12_handler_atribuicao.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.InputElement>(doc, parentRenderNode, 'input');
    final _el_1 = import7.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_2 = import7.appendText(_el_1, 't');
    final _el_3 = import7.appendElement<import6.AnchorElement>(doc, parentRenderNode, 'a');
    final _text_4 = import7.appendText(_el_3, 's');
    final _el_5 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_6 = import7.appendText(_el_5, 'i');
    final _el_7 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_8 = import7.appendText(_el_7, 'm');
    _el_0.addEventListener('input', this.eventHandler1(this._handleEvent_0));
    _el_1.addEventListener('click', this.eventHandler1(this._handleEvent_1));
    _el_3.addEventListener('click', this.eventHandler0(_ctx.salvar));
    _el_5.addEventListener('click', this.eventHandler1(_ctx.ir));
    _el_7.addEventListener('click', this.eventHandler1(this._handleEvent_2));
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.valor = $event.target.value;
  }

  void _handleEvent_1($event) {
    final _ctx = this.ctx;
    _ctx.aberto = (!_ctx.aberto);
  }

  void _handleEvent_2($event) {
    final _ctx = this.ctx;
    _ctx.mover(1, _ctx.valor);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$C12HandlerAtribuicao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C12HandlerAtribuicaoNgFactory = ComponentFactory<import1.C12HandlerAtribuicao>('c12-handler-atribuicao', viewFactory_C12HandlerAtribuicaoHost0);
ComponentFactory<import1.C12HandlerAtribuicao> get C12HandlerAtribuicaoNgFactory {
  return _C12HandlerAtribuicaoNgFactory;
}

ComponentFactory<import1.C12HandlerAtribuicao> createC12HandlerAtribuicaoFactory() {
  return ComponentFactory('c12-handler-atribuicao', viewFactory_C12HandlerAtribuicaoHost0);
}

final List<Object> styles$C12HandlerAtribuicaoHost = const [];

class _ViewC12HandlerAtribuicaoHost0 extends import9.HostView<import1.C12HandlerAtribuicao> {
  @override
  void build() {
    this.componentView = ViewC12HandlerAtribuicao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C12HandlerAtribuicao();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.C12HandlerAtribuicao> viewFactory_C12HandlerAtribuicaoHost0() {
  return _ViewC12HandlerAtribuicaoHost0();
}
