// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j52_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j52_filho.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_template_outlet.dart' as import3;
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
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$J52Filho = const [];

class ViewJ52Filho0 extends import0.ComponentView<import1.J52Filho> {
  late final ViewContainer _appEl_0;
  late final import3.NgTemplateOutlet _NgTemplateOutlet_0_9;
  Object? _expr_0;
  Object? _expr_1;
  static import4.ComponentStyles? _componentStyles;
  ViewJ52Filho0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j52-filho'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j52_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J52Filho1);
    this._NgTemplateOutlet_0_9 = import3.NgTemplateOutlet(this._appEl_0);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgTemplateOutlet_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.modelo;
    if (import12.checkBinding(this._expr_0, currVal_0, 'modelo', 'package:corpus_ngdart/src/j52_filho.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_0_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_0_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/j52_filho.html:14:59 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.contexto;
    if (import12.checkBinding(this._expr_1, currVal_1, 'contexto', 'package:corpus_ngdart/src/j52_filho.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_0_9, 'ngTemplateOutletContext', currVal_1);
      }
      this._NgTemplateOutlet_0_9.ngTemplateOutletContext = currVal_1 /* REF:package:corpus_ngdart/src/j52_filho.html:14:59 */;
      this._expr_1 = currVal_1;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgTemplateOutlet_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J52Filho, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J52FilhoNgFactory = ComponentFactory<import1.J52Filho>('j52-filho', viewFactory_J52FilhoHost0);
ComponentFactory<import1.J52Filho> get J52FilhoNgFactory {
  return _J52FilhoNgFactory;
}

ComponentFactory<import1.J52Filho> createJ52FilhoFactory() {
  return ComponentFactory('j52-filho', viewFactory_J52FilhoHost0);
}

class _ViewJ52Filho1 extends import14.EmbeddedView<import1.J52Filho> {
  _ViewJ52Filho1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import7.unsafeCast(const <Object>[]), null);
  }
}

import14.EmbeddedView<void> viewFactory_J52Filho1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ52Filho1(parentView, parentIndex);
}

final List<Object> styles$J52FilhoHost = const [];

class _ViewJ52FilhoHost0 extends import16.HostView<import1.J52Filho> {
  @override
  void build() {
    this.componentView = ViewJ52Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J52Filho();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J52Filho> viewFactory_J52FilhoHost0() {
  return _ViewJ52FilhoHost0();
}
