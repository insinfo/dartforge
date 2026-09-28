// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j77_consulta_em_projetado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j77_consulta_em_projetado.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/runtime/queries.dart' as import15;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;

final List<Object> styles$J77Modal = const [];

class ViewJ77Modal0 extends import0.ComponentView<import1.J77Modal> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ77Modal0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j77-modal'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j77_consulta_em_projetado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'corpo');
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J77Modal, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J77ModalNgFactory = ComponentFactory<import1.J77Modal>('j77-modal', viewFactory_J77ModalHost0);
ComponentFactory<import1.J77Modal> get J77ModalNgFactory {
  return _J77ModalNgFactory;
}

ComponentFactory<import1.J77Modal> createJ77ModalFactory() {
  return ComponentFactory('j77-modal', viewFactory_J77ModalHost0);
}

final List<Object> styles$J77ModalHost = const [];

class _ViewJ77ModalHost0 extends import9.HostView<import1.J77Modal> {
  @override
  void build() {
    this.componentView = ViewJ77Modal0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J77Modal();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J77Modal> viewFactory_J77ModalHost0() {
  return _ViewJ77ModalHost0();
}

final List<Object> styles$J77Tabela = const [];

class ViewJ77Tabela0 extends import0.ComponentView<import1.J77Tabela> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ77Tabela0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j77-tabela'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j77_consulta_em_projetado.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J77Tabela, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J77TabelaNgFactory = ComponentFactory<import1.J77Tabela>('j77-tabela', viewFactory_J77TabelaHost0);
ComponentFactory<import1.J77Tabela> get J77TabelaNgFactory {
  return _J77TabelaNgFactory;
}

ComponentFactory<import1.J77Tabela> createJ77TabelaFactory() {
  return ComponentFactory('j77-tabela', viewFactory_J77TabelaHost0);
}

final List<Object> styles$J77TabelaHost = const [];

class _ViewJ77TabelaHost0 extends import9.HostView<import1.J77Tabela> {
  @override
  void build() {
    this.componentView = ViewJ77Tabela0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J77Tabela();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J77Tabela> viewFactory_J77TabelaHost0() {
  return _ViewJ77TabelaHost0();
}

final List<Object> styles$J77ConsultaEmProjetado = const [];

class ViewJ77ConsultaEmProjetado0 extends import0.ComponentView<import1.J77ConsultaEmProjetado> {
  bool _viewQuery_tabela_1_isDirty = true;
  bool _viewQuery_rodape_2_isDirty = true;
  late final ViewJ77Modal0 _compView_0;
  late final import1.J77Modal _J77Modal_0_5;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ77ConsultaEmProjetado0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j77-consulta-em-projetado'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j77_consulta_em_projetado.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ77Modal0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J77Modal_0_5 = import1.J77Modal();
    final _anchor_1 = import7.createAnchor();
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J77ConsultaEmProjetado1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    final _anchor_2 = import7.createAnchor();
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J77ConsultaEmProjetado2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    this._compView_0.createAndProject(this._J77Modal_0_5, [
      <Object>[this._appEl_1, this._appEl_2]
    ]);
    _ctx.modal = this._J77Modal_0_5;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_1_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j77_consulta_em_projetado.html:33:48 */;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', (!_ctx.mostrar));
    }
    this._NgIf_2_9.ngIf = (!_ctx.mostrar) /* REF:package:corpus_ngdart/src/j77_consulta_em_projetado.html:78:94 */;
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    if ((!import14.debugThrowIfChanged)) {
      if (this._viewQuery_tabela_1_isDirty) {
        _ctx.tabela = import15.firstOrNull(this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ77ConsultaEmProjetado1 nestedView) {
          import3.View.queryChangeDetectorRefs[nestedView._J77Tabela_0_5] = nestedView._compView_0;
          return nestedView._J77Tabela_0_5;
        }));
        this._viewQuery_tabela_1_isDirty = false;
      }
      if (this._viewQuery_rodape_2_isDirty) {
        _ctx.rodape = import15.firstOrNull(this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ77ConsultaEmProjetado2 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_rodape_2_isDirty = false;
      }
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J77ConsultaEmProjetado, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J77ConsultaEmProjetadoNgFactory = ComponentFactory<import1.J77ConsultaEmProjetado>('j77-consulta-em-projetado', viewFactory_J77ConsultaEmProjetadoHost0);
ComponentFactory<import1.J77ConsultaEmProjetado> get J77ConsultaEmProjetadoNgFactory {
  return _J77ConsultaEmProjetadoNgFactory;
}

ComponentFactory<import1.J77ConsultaEmProjetado> createJ77ConsultaEmProjetadoFactory() {
  return ComponentFactory('j77-consulta-em-projetado', viewFactory_J77ConsultaEmProjetadoHost0);
}

class _ViewJ77ConsultaEmProjetado1 extends import16.EmbeddedView<import1.J77ConsultaEmProjetado> {
  late final ViewJ77Tabela0 _compView_0;
  late final import1.J77Tabela _J77Tabela_0_5;
  _ViewJ77ConsultaEmProjetado1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ77Tabela0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J77Tabela_0_5 = import1.J77Tabela();
    this._compView_0.create(this._J77Tabela_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ77ConsultaEmProjetado0>((this.parentView!))._viewQuery_tabela_1_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import16.EmbeddedView<void> viewFactory_J77ConsultaEmProjetado1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ77ConsultaEmProjetado1(parentView, parentIndex);
}

class _ViewJ77ConsultaEmProjetado2 extends import16.EmbeddedView<import1.J77ConsultaEmProjetado> {
  late final import6.HtmlElement _el_1;
  _ViewJ77ConsultaEmProjetado2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    this._el_1 = import7.appendElement<import6.HtmlElement>(doc, _el_0, 'p');
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ77ConsultaEmProjetado0>((this.parentView!))._viewQuery_rodape_2_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J77ConsultaEmProjetado2(import17.RenderView parentView, int parentIndex) {
  return _ViewJ77ConsultaEmProjetado2(parentView, parentIndex);
}

final List<Object> styles$J77ConsultaEmProjetadoHost = const [];

class _ViewJ77ConsultaEmProjetadoHost0 extends import9.HostView<import1.J77ConsultaEmProjetado> {
  @override
  void build() {
    this.componentView = ViewJ77ConsultaEmProjetado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J77ConsultaEmProjetado();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J77ConsultaEmProjetado> viewFactory_J77ConsultaEmProjetadoHost0() {
  return _ViewJ77ConsultaEmProjetadoHost0();
}
