// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j78_consulta_em_template_ngif.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j78_consulta_em_template_ngif.dart' as import1;
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

final List<Object> styles$J78Painel = const [];

class ViewJ78Painel0 extends import0.ComponentView<import1.J78Painel> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ78Painel0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j78-painel'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j78_consulta_em_template_ngif.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J78Painel, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J78PainelNgFactory = ComponentFactory<import1.J78Painel>('j78-painel', viewFactory_J78PainelHost0);
ComponentFactory<import1.J78Painel> get J78PainelNgFactory {
  return _J78PainelNgFactory;
}

ComponentFactory<import1.J78Painel> createJ78PainelFactory() {
  return ComponentFactory('j78-painel', viewFactory_J78PainelHost0);
}

final List<Object> styles$J78PainelHost = const [];

class _ViewJ78PainelHost0 extends import9.HostView<import1.J78Painel> {
  @override
  void build() {
    this.componentView = ViewJ78Painel0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J78Painel();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    if (changed) {
      this.componentView.markAsCheckOnce();
    }
    this.componentView.detectChanges();
  }
}

import9.HostView<import1.J78Painel> viewFactory_J78PainelHost0() {
  return _ViewJ78PainelHost0();
}

final List<Object> styles$J78ConsultaEmTemplateNgif = const [];

class ViewJ78ConsultaEmTemplateNgif0 extends import0.ComponentView<import1.J78ConsultaEmTemplateNgif> {
  bool _viewQuery_painel_0_isDirty = true;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ78ConsultaEmTemplateNgif0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j78-consulta-em-template-ngif'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j78_consulta_em_template_ngif.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _anchor_1 = import7.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J78ConsultaEmTemplateNgif1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_1_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j78_consulta_em_template_ngif.html:18:34 */;
    this._appEl_1.detectChangesInNestedViews();
    if ((!import14.debugThrowIfChanged)) {
      if (this._viewQuery_painel_0_isDirty) {
        _ctx.painel = import15.firstOrNull(this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ78ConsultaEmTemplateNgif1 nestedView) {
          import3.View.queryChangeDetectorRefs[nestedView._J78Painel_0_5] = nestedView._compView_0;
          return nestedView._J78Painel_0_5;
        }));
        this._viewQuery_painel_0_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J78ConsultaEmTemplateNgif, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J78ConsultaEmTemplateNgifNgFactory = ComponentFactory<import1.J78ConsultaEmTemplateNgif>('j78-consulta-em-template-ngif', viewFactory_J78ConsultaEmTemplateNgifHost0);
ComponentFactory<import1.J78ConsultaEmTemplateNgif> get J78ConsultaEmTemplateNgifNgFactory {
  return _J78ConsultaEmTemplateNgifNgFactory;
}

ComponentFactory<import1.J78ConsultaEmTemplateNgif> createJ78ConsultaEmTemplateNgifFactory() {
  return ComponentFactory('j78-consulta-em-template-ngif', viewFactory_J78ConsultaEmTemplateNgifHost0);
}

class _ViewJ78ConsultaEmTemplateNgif1 extends import16.EmbeddedView<import1.J78ConsultaEmTemplateNgif> {
  late final ViewJ78Painel0 _compView_0;
  late final import1.J78Painel _J78Painel_0_5;
  _ViewJ78ConsultaEmTemplateNgif1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ78Painel0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J78Painel_0_5 = import1.J78Painel();
    this._compView_0.create(this._J78Painel_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    if (firstCheck) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J78Painel_0_5, 'titulo', 't');
      }
      this._J78Painel_0_5.titulo = 't' /* REF:package:corpus_ngdart/src/j78_consulta_em_template_ngif.html:60:74 */;
      changed = true;
    }
    if (changed) {
      this._compView_0.markAsCheckOnce();
    }
    this._compView_0.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ78ConsultaEmTemplateNgif0>((this.parentView!))._viewQuery_painel_0_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import16.EmbeddedView<void> viewFactory_J78ConsultaEmTemplateNgif1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ78ConsultaEmTemplateNgif1(parentView, parentIndex);
}

final List<Object> styles$J78ConsultaEmTemplateNgifHost = const [];

class _ViewJ78ConsultaEmTemplateNgifHost0 extends import9.HostView<import1.J78ConsultaEmTemplateNgif> {
  @override
  void build() {
    this.componentView = ViewJ78ConsultaEmTemplateNgif0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J78ConsultaEmTemplateNgif();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J78ConsultaEmTemplateNgif> viewFactory_J78ConsultaEmTemplateNgifHost0() {
  return _ViewJ78ConsultaEmTemplateNgifHost0();
}
