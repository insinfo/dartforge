// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j92_filho_com_ligacoes_do_elemento.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j92_filho_com_ligacoes_do_elemento.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/src/runtime/interpolate.dart' as import12;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import13;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import14;

final List<Object> styles$J92Filho = const [];

class ViewJ92Filho0 extends import0.ComponentView<import1.J92Filho> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ92Filho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j92-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j92_filho_com_ligacoes_do_elemento.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J92Filho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J92FilhoNgFactory = ComponentFactory<import1.J92Filho>('j92-filho', viewFactory_J92FilhoHost0);
ComponentFactory<import1.J92Filho> get J92FilhoNgFactory {
  return _J92FilhoNgFactory;
}

ComponentFactory<import1.J92Filho> createJ92FilhoFactory() {
  return ComponentFactory('j92-filho', viewFactory_J92FilhoHost0);
}

final List<Object> styles$J92FilhoHost = const [];

class _ViewJ92FilhoHost0 extends import9.HostView<import1.J92Filho> {
  @override
  void build() {
    this.componentView = ViewJ92Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J92Filho();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J92Filho> viewFactory_J92FilhoHost0() {
  return _ViewJ92FilhoHost0();
}

final List<Object> styles$J92FilhoComLigacoesDoElemento = const [];

class ViewJ92FilhoComLigacoesDoElemento0 extends import0.ComponentView<import1.J92FilhoComLigacoesDoElemento> {
  late final ViewJ92Filho0 _compView_0;
  late final import1.J92Filho _J92Filho_0_5;
  late final ViewJ92Filho0 _compView_1;
  late final import1.J92Filho _J92Filho_1_5;
  late final J92DestaqueNgCd _J92Destaque_1_6;
  Object? _expr_0;
  Object? _expr_1;
  late final import6.HtmlElement _el_0;
  late final import6.HtmlElement _el_1;
  static import2.ComponentStyles? _componentStyles;
  ViewJ92FilhoComLigacoesDoElemento0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j92-filho-com-ligacoes-do-elemento'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j92_filho_com_ligacoes_do_elemento.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ92Filho0(this, 0);
    this._el_0 = this._compView_0.rootElement;
    parentRenderNode.append(this._el_0);
    this._J92Filho_0_5 = import1.J92Filho();
    this._compView_0.create(this._J92Filho_0_5);
    this._compView_1 = ViewJ92Filho0(this, 1);
    this._el_1 = this._compView_1.rootElement;
    parentRenderNode.append(this._el_1);
    import7.setAttribute(this._el_1, 'j92-destaque', '');
    this._J92Filho_1_5 = import1.J92Filho();
    this._J92Destaque_1_6 = J92DestaqueNgCd(import1.J92Destaque());
    if (import10.isDevToolsEnabled) {
      import10.Inspector.instance.registerDirective(this._el_1, this._J92Destaque_1_6.instance);
    }
    this._compView_1.create(this._J92Filho_1_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_1 = _ctx.nome;
    if (import11.checkBinding(this._expr_1, currVal_1, 'nome', 'asset:corpus_ngdart/lib/src/j92_filho_com_ligacoes_do_elemento.dart')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._J92Filho_0_5, 'rotulo', currVal_1);
      }
      this._J92Filho_0_5.rotulo = currVal_1 /* REF:asset:corpus_ngdart/lib/src/j92_filho_com_ligacoes_do_elemento.dart:557:572 */;
      this._expr_1 = currVal_1;
    }
    final currVal_0 = import12.interpolateString1('a ', _ctx.nome, '');
    if (import11.checkBinding(this._expr_0, currVal_0, 'a {{ nome }}', 'asset:corpus_ngdart/lib/src/j92_filho_com_ligacoes_do_elemento.dart')) {
      import7.setProperty(this._el_0, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j92_filho_com_ligacoes_do_elemento.dart:536:556 */;
      this._expr_0 = currVal_0;
    }
    this._J92Destaque_1_6.detectHostChanges(this._compView_1, this._el_1);
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J92FilhoComLigacoesDoElemento, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J92FilhoComLigacoesDoElementoNgFactory = ComponentFactory<import1.J92FilhoComLigacoesDoElemento>('j92-filho-com-ligacoes-do-elemento', viewFactory_J92FilhoComLigacoesDoElementoHost0);
ComponentFactory<import1.J92FilhoComLigacoesDoElemento> get J92FilhoComLigacoesDoElementoNgFactory {
  return _J92FilhoComLigacoesDoElementoNgFactory;
}

ComponentFactory<import1.J92FilhoComLigacoesDoElemento> createJ92FilhoComLigacoesDoElementoFactory() {
  return ComponentFactory('j92-filho-com-ligacoes-do-elemento', viewFactory_J92FilhoComLigacoesDoElementoHost0);
}

final List<Object> styles$J92FilhoComLigacoesDoElementoHost = const [];

class _ViewJ92FilhoComLigacoesDoElementoHost0 extends import9.HostView<import1.J92FilhoComLigacoesDoElemento> {
  @override
  void build() {
    this.componentView = ViewJ92FilhoComLigacoesDoElemento0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J92FilhoComLigacoesDoElemento();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J92FilhoComLigacoesDoElemento> viewFactory_J92FilhoComLigacoesDoElementoHost0() {
  return _ViewJ92FilhoComLigacoesDoElementoHost0();
}

class J92DestaqueNgCd extends import13.DirectiveChangeDetector {
  final import1.J92Destaque instance;
  Object? _expr_0;
  Object? _expr_1;
  J92DestaqueNgCd(this.instance);
  void detectHostChanges(import14.RenderView view, import6.Element el) {
    final currVal_0 = this.instance.ativo;
    if (import11.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(el, 'destaque', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = this.instance.rotulo;
    if (import11.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateAttribute(el, 'aria-label', currVal_1);
      this._expr_1 = currVal_1;
    }
  }
}
