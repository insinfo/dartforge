// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i97_view_child_dois_niveis.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i97_view_child_dois_niveis.dart' as import1;
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

final List<Object> styles$I97ViewChildDoisNiveis = const [];

class ViewI97ViewChildDoisNiveis0 extends import0.ComponentView<import1.I97ViewChildDoisNiveis> {
  bool _viewQuery_fundo_0_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewI97ViewChildDoisNiveis0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i97-view-child-dois-niveis'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i97_view_child_dois_niveis.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I97ViewChildDoisNiveis1);
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
    this._NgIf_0_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/i97_view_child_dois_niveis.html:5:14 */;
    this._appEl_0.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_fundo_0_isDirty) {
        _ctx.fundo = import13.firstOrNull(this._appEl_0.mapNestedViews((_ViewI97ViewChildDoisNiveis1 nestedView) {
          return nestedView._appEl_1.mapNestedViewsWithSingleResult((_ViewI97ViewChildDoisNiveis2 nestedView) {
            return nestedView._el_1;
          });
        }));
        this._viewQuery_fundo_0_isDirty = false;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I97ViewChildDoisNiveis, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I97ViewChildDoisNiveisNgFactory = ComponentFactory<import1.I97ViewChildDoisNiveis>('i97-view-child-dois-niveis', viewFactory_I97ViewChildDoisNiveisHost0);
ComponentFactory<import1.I97ViewChildDoisNiveis> get I97ViewChildDoisNiveisNgFactory {
  return _I97ViewChildDoisNiveisNgFactory;
}

ComponentFactory<import1.I97ViewChildDoisNiveis> createI97ViewChildDoisNiveisFactory() {
  return ComponentFactory('i97-view-child-dois-niveis', viewFactory_I97ViewChildDoisNiveisHost0);
}

class _ViewI97ViewChildDoisNiveis1 extends import15.EmbeddedView<import1.I97ViewChildDoisNiveis> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewI97ViewChildDoisNiveis1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I97ViewChildDoisNiveis2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.b);
    }
    this._NgIf_1_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/i97_view_child_dois_niveis.html:20:29 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_I97ViewChildDoisNiveis1(import16.RenderView parentView, int parentIndex) {
  return _ViewI97ViewChildDoisNiveis1(parentView, parentIndex);
}

class _ViewI97ViewChildDoisNiveis2 extends import15.EmbeddedView<import1.I97ViewChildDoisNiveis> {
  late final import8.HtmlElement _el_1;
  _ViewI97ViewChildDoisNiveis2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
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
    import7.unsafeCast<ViewI97ViewChildDoisNiveis0>(((this.parentView!).parentView!))._viewQuery_fundo_0_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_I97ViewChildDoisNiveis2(import16.RenderView parentView, int parentIndex) {
  return _ViewI97ViewChildDoisNiveis2(parentView, parentIndex);
}

final List<Object> styles$I97ViewChildDoisNiveisHost = const [];

class _ViewI97ViewChildDoisNiveisHost0 extends import17.HostView<import1.I97ViewChildDoisNiveis> {
  @override
  void build() {
    this.componentView = ViewI97ViewChildDoisNiveis0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I97ViewChildDoisNiveis();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.I97ViewChildDoisNiveis> viewFactory_I97ViewChildDoisNiveisHost0() {
  return _ViewI97ViewChildDoisNiveisHost0();
}
