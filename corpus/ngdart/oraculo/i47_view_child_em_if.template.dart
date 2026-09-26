// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i47_view_child_em_if.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i47_view_child_em_if.dart' as import1;
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

final List<Object> styles$I47ViewChildEmIf = const [];

class ViewI47ViewChildEmIf0 extends import0.ComponentView<import1.I47ViewChildEmIf> {
  bool _viewQuery_caixa_0_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewI47ViewChildEmIf0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i47-view-child-em-if'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i47_view_child_em_if.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I47ViewChildEmIf1);
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
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i47_view_child_em_if.html:5:20 */;
    this._appEl_0.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_caixa_0_isDirty) {
        _ctx.caixa = import13.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewI47ViewChildEmIf1 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_caixa_0_isDirty = false;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I47ViewChildEmIf, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I47ViewChildEmIfNgFactory = ComponentFactory<import1.I47ViewChildEmIf>('i47-view-child-em-if', viewFactory_I47ViewChildEmIfHost0);
ComponentFactory<import1.I47ViewChildEmIf> get I47ViewChildEmIfNgFactory {
  return _I47ViewChildEmIfNgFactory;
}

ComponentFactory<import1.I47ViewChildEmIf> createI47ViewChildEmIfFactory() {
  return ComponentFactory('i47-view-child-em-if', viewFactory_I47ViewChildEmIfHost0);
}

class _ViewI47ViewChildEmIf1 extends import15.EmbeddedView<import1.I47ViewChildEmIf> {
  late final import8.HtmlElement _el_1;
  _ViewI47ViewChildEmIf1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this._el_1 = import9.appendSpan(doc, _el_0);
    final _text_2 = import9.appendText(this._el_1, 'x');
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewI47ViewChildEmIf0>((this.parentView!))._viewQuery_caixa_0_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_I47ViewChildEmIf1(import16.RenderView parentView, int parentIndex) {
  return _ViewI47ViewChildEmIf1(parentView, parentIndex);
}

final List<Object> styles$I47ViewChildEmIfHost = const [];

class _ViewI47ViewChildEmIfHost0 extends import17.HostView<import1.I47ViewChildEmIf> {
  @override
  void build() {
    this.componentView = ViewI47ViewChildEmIf0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I47ViewChildEmIf();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.I47ViewChildEmIf> viewFactory_I47ViewChildEmIfHost0() {
  return _ViewI47ViewChildEmIfHost0();
}
