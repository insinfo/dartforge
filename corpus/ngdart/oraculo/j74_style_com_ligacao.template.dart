// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j74_style_com_ligacao.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j74_style_com_ligacao.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;

final List<Object> styles$J74Filho = const [];

class ViewJ74Filho0 extends import0.ComponentView<import1.J74Filho> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ74Filho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j74-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j74_style_com_ligacao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J74Filho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J74FilhoNgFactory = ComponentFactory<import1.J74Filho>('j74-filho', viewFactory_J74FilhoHost0);
ComponentFactory<import1.J74Filho> get J74FilhoNgFactory {
  return _J74FilhoNgFactory;
}

ComponentFactory<import1.J74Filho> createJ74FilhoFactory() {
  return ComponentFactory('j74-filho', viewFactory_J74FilhoHost0);
}

final List<Object> styles$J74FilhoHost = const [];

class _ViewJ74FilhoHost0 extends import9.HostView<import1.J74Filho> {
  @override
  void build() {
    this.componentView = ViewJ74Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J74Filho();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J74Filho> viewFactory_J74FilhoHost0() {
  return _ViewJ74FilhoHost0();
}

final List<Object> styles$J74StyleComLigacao = const [];

class ViewJ74StyleComLigacao0 extends import0.ComponentView<import1.J74StyleComLigacao> {
  late final ViewJ74Filho0 _compView_1;
  late final import1.J74Filho _J74Filho_1_5;
  late final ViewJ74Filho0 _compView_2;
  late final import1.J74Filho _J74Filho_2_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  Object? _expr_4;
  Object? _expr_5;
  late final import6.DivElement _el_0;
  late final import6.HtmlElement _el_1;
  late final import6.HtmlElement _el_2;
  static import2.ComponentStyles? _componentStyles;
  ViewJ74StyleComLigacao0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j74-style-com-ligacao'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j74_style_com_ligacao.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    import7.setAttribute(this._el_0, 'style', 'margin: auto;');
    this._compView_1 = ViewJ74Filho0(this, 1);
    this._el_1 = this._compView_1.rootElement;
    parentRenderNode.append(this._el_1);
    import7.setAttribute(this._el_1, 'style', 'display: block');
    this._J74Filho_1_5 = import1.J74Filho();
    this._compView_1.create(this._J74Filho_1_5);
    this._compView_2 = ViewJ74Filho0(this, 2);
    this._el_2 = this._compView_2.rootElement;
    parentRenderNode.append(this._el_2);
    this.updateChildClassNonHtml(this._el_2, 'fixa');
    this._J74Filho_2_5 = import1.J74Filho();
    this._compView_2.create(this._J74Filho_2_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.topo;
    if (import10.checkBinding(this._expr_0, currVal_0, 'topo', 'package:corpus_ngdart/src/j74_style_com_ligacao.html')) {
      this._el_0.style.setProperty('top', currVal_0) /* REF:package:corpus_ngdart/src/j74_style_com_ligacao.html:27:45 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.largura;
    if (import10.checkBinding(this._expr_1, currVal_1, 'largura', 'package:corpus_ngdart/src/j74_style_com_ligacao.html')) {
      this._el_0.style.setProperty('width', ((currVal_1 == null) ? null : (currVal_1.toString() + 'px'))) /* REF:package:corpus_ngdart/src/j74_style_com_ligacao.html:46:72 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.topo;
    if (import10.checkBinding(this._expr_2, currVal_2, 'topo', 'package:corpus_ngdart/src/j74_style_com_ligacao.html')) {
      this._el_1.style.setProperty('top', currVal_2) /* REF:package:corpus_ngdart/src/j74_style_com_ligacao.html:114:132 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = _ctx.ativo;
    if (import10.checkBinding(this._expr_3, currVal_3, 'ativo', 'package:corpus_ngdart/src/j74_style_com_ligacao.html')) {
      import7.updateClassBindingNonHtml(this._el_2, 'ativo', currVal_3) /* REF:package:corpus_ngdart/src/j74_style_com_ligacao.html:170:191 */;
      this._expr_3 = currVal_3;
    }
    final currVal_4 = _ctx.extra;
    if (import10.checkBinding(this._expr_4, currVal_4, 'extra', 'package:corpus_ngdart/src/j74_style_com_ligacao.html')) {
      import7.updateAttribute(this._el_2, 'data-x', currVal_4) /* REF:package:corpus_ngdart/src/j74_style_com_ligacao.html:192:213 */;
      this._expr_4 = currVal_4;
    }
    final currVal_5 = _ctx.extra;
    if (import10.checkBinding(this._expr_5, currVal_5, 'extra', 'package:corpus_ngdart/src/j74_style_com_ligacao.html')) {
      this._compView_2.updateChildClassNonHtml(this._el_2, currVal_5) /* REF:package:corpus_ngdart/src/j74_style_com_ligacao.html:214:229 */;
      this._expr_5 = currVal_5;
    }
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J74StyleComLigacao, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J74StyleComLigacaoNgFactory = ComponentFactory<import1.J74StyleComLigacao>('j74-style-com-ligacao', viewFactory_J74StyleComLigacaoHost0);
ComponentFactory<import1.J74StyleComLigacao> get J74StyleComLigacaoNgFactory {
  return _J74StyleComLigacaoNgFactory;
}

ComponentFactory<import1.J74StyleComLigacao> createJ74StyleComLigacaoFactory() {
  return ComponentFactory('j74-style-com-ligacao', viewFactory_J74StyleComLigacaoHost0);
}

final List<Object> styles$J74StyleComLigacaoHost = const [];

class _ViewJ74StyleComLigacaoHost0 extends import9.HostView<import1.J74StyleComLigacao> {
  @override
  void build() {
    this.componentView = ViewJ74StyleComLigacao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J74StyleComLigacao();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J74StyleComLigacao> viewFactory_J74StyleComLigacaoHost0() {
  return _ViewJ74StyleComLigacaoHost0();
}
