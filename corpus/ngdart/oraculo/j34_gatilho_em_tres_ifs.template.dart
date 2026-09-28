// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j34_gatilho_em_tres_ifs.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j34_gatilho_em_tres_ifs.dart' as import1;
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

final List<Object> styles$J34GatilhoEmTresIfs = const [];

class ViewJ34GatilhoEmTresIfs0 extends import0.ComponentView<import1.J34GatilhoEmTresIfs> {
  bool _viewQuery_gatilho_0_isDirty = true;
  bool _viewQuery_gatilho_1_isDirty = true;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ34GatilhoEmTresIfs0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j34-gatilho-em-tres-ifs'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j34_gatilho_em_tres_ifs.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'caixa');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J34GatilhoEmTresIfs1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    final _anchor_2 = import9.appendAnchor(_el_0);
    this._appEl_2 = ViewContainer(2, 0, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J34GatilhoEmTresIfs2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    final _anchor_3 = import9.appendAnchor(_el_0);
    this._appEl_3 = ViewContainer(3, 0, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J34GatilhoEmTresIfs3);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.a);
    }
    this._NgIf_1_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j34_gatilho_em_tres_ifs.html:27:36 */;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.b);
    }
    this._NgIf_2_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/j34_gatilho_em_tres_ifs.html:90:99 */;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', ((!_ctx.a) && (!_ctx.b)));
    }
    this._NgIf_3_9.ngIf = ((!_ctx.a) && (!_ctx.b)) /* REF:package:corpus_ngdart/src/j34_gatilho_em_tres_ifs.html:155:171 */;
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_3.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_gatilho_0_isDirty) {
        _ctx.gatilho = import13.firstOrNull([
          ...this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ34GatilhoEmTresIfs1 nestedView) {
            return nestedView._el_0;
          }),
          ...this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ34GatilhoEmTresIfs2 nestedView) {
            return nestedView._el_0;
          }),
          ...this._appEl_3.mapNestedViewsWithSingleResult((_ViewJ34GatilhoEmTresIfs3 nestedView) {
            return nestedView._el_0;
          })
        ]);
        this._viewQuery_gatilho_0_isDirty = false;
      }
      if (this._viewQuery_gatilho_1_isDirty) {
        _ctx.todos = [
          ...this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ34GatilhoEmTresIfs1 nestedView) {
            return nestedView._el_0;
          }),
          ...this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ34GatilhoEmTresIfs2 nestedView) {
            return nestedView._el_0;
          }),
          ...this._appEl_3.mapNestedViewsWithSingleResult((_ViewJ34GatilhoEmTresIfs3 nestedView) {
            return nestedView._el_0;
          })
        ];
        this._viewQuery_gatilho_1_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J34GatilhoEmTresIfs, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J34GatilhoEmTresIfsNgFactory = ComponentFactory<import1.J34GatilhoEmTresIfs>('j34-gatilho-em-tres-ifs', viewFactory_J34GatilhoEmTresIfsHost0);
ComponentFactory<import1.J34GatilhoEmTresIfs> get J34GatilhoEmTresIfsNgFactory {
  return _J34GatilhoEmTresIfsNgFactory;
}

ComponentFactory<import1.J34GatilhoEmTresIfs> createJ34GatilhoEmTresIfsFactory() {
  return ComponentFactory('j34-gatilho-em-tres-ifs', viewFactory_J34GatilhoEmTresIfsHost0);
}

class _ViewJ34GatilhoEmTresIfs1 extends import15.EmbeddedView<import1.J34GatilhoEmTresIfs> {
  late final import8.DivElement _el_0;
  _ViewJ34GatilhoEmTresIfs1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _ctx = this.ctx;
    final doc = import8.document;
    this._el_0 = import7.unsafeCast(doc.createElement('div'));
    this.updateChildClass(this._el_0, 'um');
    final _text_1 = import9.appendText(this._el_0, '1');
    this._el_0.addEventListener('click', this.eventHandler0(_ctx.abrir));
    this.initRootNode(this._el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ34GatilhoEmTresIfs0>((this.parentView!))._viewQuery_gatilho_0_isDirty = true;
    import7.unsafeCast<ViewJ34GatilhoEmTresIfs0>((this.parentView!))._viewQuery_gatilho_1_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J34GatilhoEmTresIfs1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ34GatilhoEmTresIfs1(parentView, parentIndex);
}

class _ViewJ34GatilhoEmTresIfs2 extends import15.EmbeddedView<import1.J34GatilhoEmTresIfs> {
  late final import8.DivElement _el_0;
  _ViewJ34GatilhoEmTresIfs2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _ctx = this.ctx;
    final doc = import8.document;
    this._el_0 = import7.unsafeCast(doc.createElement('div'));
    this.updateChildClass(this._el_0, 'dois');
    final _text_1 = import9.appendText(this._el_0, '2');
    this._el_0.addEventListener('click', this.eventHandler0(_ctx.abrir));
    this.initRootNode(this._el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ34GatilhoEmTresIfs0>((this.parentView!))._viewQuery_gatilho_0_isDirty = true;
    import7.unsafeCast<ViewJ34GatilhoEmTresIfs0>((this.parentView!))._viewQuery_gatilho_1_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J34GatilhoEmTresIfs2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ34GatilhoEmTresIfs2(parentView, parentIndex);
}

class _ViewJ34GatilhoEmTresIfs3 extends import15.EmbeddedView<import1.J34GatilhoEmTresIfs> {
  late final import8.DivElement _el_0;
  _ViewJ34GatilhoEmTresIfs3(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    this._el_0 = import7.unsafeCast(doc.createElement('div'));
    this.updateChildClass(this._el_0, 'tres');
    final _text_1 = import9.appendText(this._el_0, '3');
    this.initRootNode(this._el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ34GatilhoEmTresIfs0>((this.parentView!))._viewQuery_gatilho_0_isDirty = true;
    import7.unsafeCast<ViewJ34GatilhoEmTresIfs0>((this.parentView!))._viewQuery_gatilho_1_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J34GatilhoEmTresIfs3(import16.RenderView parentView, int parentIndex) {
  return _ViewJ34GatilhoEmTresIfs3(parentView, parentIndex);
}

final List<Object> styles$J34GatilhoEmTresIfsHost = const [];

class _ViewJ34GatilhoEmTresIfsHost0 extends import17.HostView<import1.J34GatilhoEmTresIfs> {
  @override
  void build() {
    this.componentView = ViewJ34GatilhoEmTresIfs0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J34GatilhoEmTresIfs();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J34GatilhoEmTresIfs> viewFactory_J34GatilhoEmTresIfsHost0() {
  return _ViewJ34GatilhoEmTresIfsHost0();
}
