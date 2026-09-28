// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j66_sujas_em_ordem.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j66_sujas_em_ordem.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/src/runtime/queries.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J66SujasEmOrdem = const [];

class ViewJ66SujasEmOrdem0 extends import0.ComponentView<import1.J66SujasEmOrdem> {
  bool _viewQuery_rolagem_1_isDirty = true;
  bool _viewQuery_tabela_0_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ66SujasEmOrdem0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j66-sujas-em-ordem'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j66_sujas_em_ordem.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J66SujasEmOrdem1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j66_sujas_em_ordem.html:5:20 */;
    this._appEl_0.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_tabela_0_isDirty) {
        _ctx.tabela = import13.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ66SujasEmOrdem1 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_tabela_0_isDirty = false;
      }
      if (this._viewQuery_rolagem_1_isDirty) {
        _ctx.rolagem = import13.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ66SujasEmOrdem1 nestedView) {
          return nestedView._el_0;
        }));
        this._viewQuery_rolagem_1_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J66SujasEmOrdem, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J66SujasEmOrdemNgFactory = ComponentFactory<import1.J66SujasEmOrdem>('j66-sujas-em-ordem', viewFactory_J66SujasEmOrdemHost0);
ComponentFactory<import1.J66SujasEmOrdem> get J66SujasEmOrdemNgFactory {
  return _J66SujasEmOrdemNgFactory;
}

ComponentFactory<import1.J66SujasEmOrdem> createJ66SujasEmOrdemFactory() {
  return ComponentFactory('j66-sujas-em-ordem', viewFactory_J66SujasEmOrdemHost0);
}

class _ViewJ66SujasEmOrdem1 extends import15.EmbeddedView<import1.J66SujasEmOrdem> {
  Object? _expr_0;
  Object? _expr_1;
  late final import8.TableElement _el_1;
  late final import8.DivElement _el_0;
  _ViewJ66SujasEmOrdem1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    this._el_0 = import7.unsafeCast(doc.createElement('div'));
    this._el_1 = import9.appendElement<import8.TableElement>(doc, this._el_0, 'table');
    this.initRootNode(this._el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = (_ctx.classe == 'x');
    if (import12.checkBinding(this._expr_0, currVal_0, 'classe == \'x\'', 'package:corpus_ngdart/src/j66_sujas_em_ordem.html')) {
      import9.updateClassBinding(this._el_0, 'x', currVal_0) /* REF:package:corpus_ngdart/src/j66_sujas_em_ordem.html:30:55 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = (_ctx.classe == 'y');
    if (import12.checkBinding(this._expr_1, currVal_1, 'classe == \'y\'', 'package:corpus_ngdart/src/j66_sujas_em_ordem.html')) {
      import9.updateClassBinding(this._el_1, 'y', currVal_1) /* REF:package:corpus_ngdart/src/j66_sujas_em_ordem.html:74:99 */;
      this._expr_1 = currVal_1;
    }
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ66SujasEmOrdem0>((this.parentView!))._viewQuery_rolagem_1_isDirty = true;
    import7.unsafeCast<ViewJ66SujasEmOrdem0>((this.parentView!))._viewQuery_tabela_0_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J66SujasEmOrdem1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ66SujasEmOrdem1(parentView, parentIndex);
}

final List<Object> styles$J66SujasEmOrdemHost = const [];

class _ViewJ66SujasEmOrdemHost0 extends import17.HostView<import1.J66SujasEmOrdem> {
  @override
  void build() {
    this.componentView = ViewJ66SujasEmOrdem0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J66SujasEmOrdem();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J66SujasEmOrdem> viewFactory_J66SujasEmOrdemHost0() {
  return _ViewJ66SujasEmOrdemHost0();
}
