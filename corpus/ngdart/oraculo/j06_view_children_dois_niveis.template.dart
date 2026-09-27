// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j06_view_children_dois_niveis.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j06_view_children_dois_niveis.dart' as import1;
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
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/common/directives/ng_for.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$J06ViewChildrenDoisNiveis = const [];

class ViewJ06ViewChildrenDoisNiveis0 extends import0.ComponentView<import1.J06ViewChildrenDoisNiveis> {
  bool _viewQuery_item_0_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ06ViewChildrenDoisNiveis0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j06-view-children-dois-niveis'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j06_view_children_dois_niveis.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J06ViewChildrenDoisNiveis1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.a);
    }
    this._NgIf_0_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j06_view_children_dois_niveis.html:5:14 */;
    this._appEl_0.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_item_0_isDirty) {
        _ctx.itens = this._appEl_0.mapNestedViews((_ViewJ06ViewChildrenDoisNiveis1 nestedView) {
          return nestedView._appEl_1.mapNestedViewsWithSingleResult((_ViewJ06ViewChildrenDoisNiveis2 nestedView) {
            return nestedView._el_0;
          });
        });
        this._viewQuery_item_0_isDirty = false;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J06ViewChildrenDoisNiveis, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J06ViewChildrenDoisNiveisNgFactory = ComponentFactory<import1.J06ViewChildrenDoisNiveis>('j06-view-children-dois-niveis', viewFactory_J06ViewChildrenDoisNiveisHost0);
ComponentFactory<import1.J06ViewChildrenDoisNiveis> get J06ViewChildrenDoisNiveisNgFactory {
  return _J06ViewChildrenDoisNiveisNgFactory;
}

ComponentFactory<import1.J06ViewChildrenDoisNiveis> createJ06ViewChildrenDoisNiveisFactory() {
  return ComponentFactory('j06-view-children-dois-niveis', viewFactory_J06ViewChildrenDoisNiveisHost0);
}

class _ViewJ06ViewChildrenDoisNiveis1 extends import14.EmbeddedView<import1.J06ViewChildrenDoisNiveis> {
  late final ViewContainer _appEl_1;
  late final import15.NgFor _NgFor_1_9;
  Object? _expr_0;
  _ViewJ06ViewChildrenDoisNiveis1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J06ViewChildrenDoisNiveis2);
    this._NgFor_1_9 = import15.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.xs;
    if (import12.checkBinding(this._expr_0, currVal_0, 'xs', 'package:corpus_ngdart/src/j06_view_children_dois_niveis.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j06_view_children_dois_niveis.html:18:38 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import14.EmbeddedView<void> viewFactory_J06ViewChildrenDoisNiveis1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ06ViewChildrenDoisNiveis1(parentView, parentIndex);
}

class _ViewJ06ViewChildrenDoisNiveis2 extends import14.EmbeddedView<import1.J06ViewChildrenDoisNiveis> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  late final import8.HtmlElement _el_0;
  _ViewJ06ViewChildrenDoisNiveis2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    this._el_0 = import7.unsafeCast(doc.createElement('p'));
    this._el_0.append(this._textBinding_1.element);
    this.initRootNode(this._el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = import7.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import19.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/j06_view_children_dois_niveis.html:45:50 */;
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ06ViewChildrenDoisNiveis0>(((this.parentView!).parentView!))._viewQuery_item_0_isDirty = true;
  }
}

import14.EmbeddedView<void> viewFactory_J06ViewChildrenDoisNiveis2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ06ViewChildrenDoisNiveis2(parentView, parentIndex);
}

final List<Object> styles$J06ViewChildrenDoisNiveisHost = const [];

class _ViewJ06ViewChildrenDoisNiveisHost0 extends import20.HostView<import1.J06ViewChildrenDoisNiveis> {
  @override
  void build() {
    this.componentView = ViewJ06ViewChildrenDoisNiveis0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J06ViewChildrenDoisNiveis();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.J06ViewChildrenDoisNiveis> viewFactory_J06ViewChildrenDoisNiveisHost0() {
  return _ViewJ06ViewChildrenDoisNiveisHost0();
}
