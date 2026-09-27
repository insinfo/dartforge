// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j09_view_child_filho_em_if.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j09_view_child_filho_em_if.dart' as import1;
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
import 'a02_texto_estatico.template.dart' as import16;
import 'a02_texto_estatico.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J09ViewChildFilhoEmIf = const [];

class ViewJ09ViewChildFilhoEmIf0 extends import0.ComponentView<import1.J09ViewChildFilhoEmIf> {
  bool _viewQuery_f_0_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ09ViewChildFilhoEmIf0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j09-view-child-filho-em-if'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j09_view_child_filho_em_if.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J09ViewChildFilhoEmIf1);
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
    this._NgIf_0_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j09_view_child_filho_em_if.html:5:14 */;
    this._appEl_0.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_f_0_isDirty) {
        _ctx.filho = import13.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ09ViewChildFilhoEmIf1 nestedView) {
          return nestedView._A02TextoEstatico_1_5;
        }));
        this._viewQuery_f_0_isDirty = false;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J09ViewChildFilhoEmIf, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J09ViewChildFilhoEmIfNgFactory = ComponentFactory<import1.J09ViewChildFilhoEmIf>('j09-view-child-filho-em-if', viewFactory_J09ViewChildFilhoEmIfHost0);
ComponentFactory<import1.J09ViewChildFilhoEmIf> get J09ViewChildFilhoEmIfNgFactory {
  return _J09ViewChildFilhoEmIfNgFactory;
}

ComponentFactory<import1.J09ViewChildFilhoEmIf> createJ09ViewChildFilhoEmIfFactory() {
  return ComponentFactory('j09-view-child-filho-em-if', viewFactory_J09ViewChildFilhoEmIfHost0);
}

class _ViewJ09ViewChildFilhoEmIf1 extends import15.EmbeddedView<import1.J09ViewChildFilhoEmIf> {
  late final import16.ViewA02TextoEstatico0 _compView_1;
  late final import17.A02TextoEstatico _A02TextoEstatico_1_5;
  _ViewJ09ViewChildFilhoEmIf1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this._compView_1 = import16.ViewA02TextoEstatico0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._A02TextoEstatico_1_5 = import17.A02TextoEstatico();
    this._compView_1.create(this._A02TextoEstatico_1_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_1.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ09ViewChildFilhoEmIf0>((this.parentView!))._viewQuery_f_0_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }
}

import15.EmbeddedView<void> viewFactory_J09ViewChildFilhoEmIf1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ09ViewChildFilhoEmIf1(parentView, parentIndex);
}

final List<Object> styles$J09ViewChildFilhoEmIfHost = const [];

class _ViewJ09ViewChildFilhoEmIfHost0 extends import19.HostView<import1.J09ViewChildFilhoEmIf> {
  @override
  void build() {
    this.componentView = ViewJ09ViewChildFilhoEmIf0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J09ViewChildFilhoEmIf();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J09ViewChildFilhoEmIf> viewFactory_J09ViewChildFilhoEmIfHost0() {
  return _ViewJ09ViewChildFilhoEmIfHost0();
}
