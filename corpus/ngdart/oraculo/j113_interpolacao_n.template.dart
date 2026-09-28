// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j113_interpolacao_n.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j113_interpolacao_n.dart' as import1;
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
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;

final List<Object> styles$J113Alvo = const [];

class ViewJ113Alvo0 extends import0.ComponentView<import1.J113Alvo> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ113Alvo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j113-alvo'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'i');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.rotulo)) /* REF:asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart:211:223 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J113Alvo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J113AlvoNgFactory = ComponentFactory<import1.J113Alvo>('j113-alvo', viewFactory_J113AlvoHost0);
ComponentFactory<import1.J113Alvo> get J113AlvoNgFactory {
  return _J113AlvoNgFactory;
}

ComponentFactory<import1.J113Alvo> createJ113AlvoFactory() {
  return ComponentFactory('j113-alvo', viewFactory_J113AlvoHost0);
}

final List<Object> styles$J113AlvoHost = const [];

class _ViewJ113AlvoHost0 extends import11.HostView<import1.J113Alvo> {
  @override
  void build() {
    this.componentView = ViewJ113Alvo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J113Alvo();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J113Alvo> viewFactory_J113AlvoHost0() {
  return _ViewJ113AlvoHost0();
}

final List<Object> styles$J113InterpolacaoN = const [];

class ViewJ113InterpolacaoN0 extends import0.ComponentView<import1.J113InterpolacaoN> {
  late final ViewJ113Alvo0 _compView_1;
  late final import1.J113Alvo _J113Alvo_1_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import7.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ113InterpolacaoN0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j113-interpolacao-n'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    this._el_0 = import8.appendDiv(doc, parentRenderNode);
    this._compView_1 = ViewJ113Alvo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J113Alvo_1_5 = import1.J113Alvo();
    this._compView_1.create(this._J113Alvo_1_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_2 = import9.interpolateN(['', _ctx.a, '', _ctx.b, '', _ctx.c, '']);
    if (import12.checkBinding(this._expr_2, currVal_2, '{{a}}{{b}}{{c}}', 'asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J113Alvo_1_5, 'rotulo', currVal_2);
      }
      this._J113Alvo_1_5.rotulo = currVal_2 /* REF:asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart:433:457 */;
      this._expr_2 = currVal_2;
    }
    final currVal_0 = import9.interpolateN(['', _ctx.a, '-', _ctx.b, '-', _ctx.c, '']);
    if (import12.checkBinding(this._expr_0, currVal_0, '{{a}}-{{b}}-{{c}}', 'asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart:349:374 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = import9.interpolateN(['x ', _ctx.a, ' ', _ctx.n, ' ', _ctx.b, ' y']);
    if (import12.checkBinding(this._expr_1, currVal_1, 'x {{a}} {{n}} {{b}} y', 'asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart')) {
      import8.setAttribute(this._el_0, 'aria-label', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j113_interpolacao_n.dart:375:414 */;
      this._expr_1 = currVal_1;
    }
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J113InterpolacaoN, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J113InterpolacaoNNgFactory = ComponentFactory<import1.J113InterpolacaoN>('j113-interpolacao-n', viewFactory_J113InterpolacaoNHost0);
ComponentFactory<import1.J113InterpolacaoN> get J113InterpolacaoNNgFactory {
  return _J113InterpolacaoNNgFactory;
}

ComponentFactory<import1.J113InterpolacaoN> createJ113InterpolacaoNFactory() {
  return ComponentFactory('j113-interpolacao-n', viewFactory_J113InterpolacaoNHost0);
}

final List<Object> styles$J113InterpolacaoNHost = const [];

class _ViewJ113InterpolacaoNHost0 extends import11.HostView<import1.J113InterpolacaoN> {
  @override
  void build() {
    this.componentView = ViewJ113InterpolacaoN0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J113InterpolacaoN();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J113InterpolacaoN> viewFactory_J113InterpolacaoNHost0() {
  return _ViewJ113InterpolacaoNHost0();
}
