// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i56_usa_host_binding.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i56_usa_host_binding.dart' as import1;
import 'i55_host_binding_formas.template.dart' as import2;
import 'i55_host_binding_formas.dart' as import3;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$I56UsaHostBinding = const [];

class ViewI56UsaHostBinding0 extends import0.ComponentView<import1.I56UsaHostBinding> {
  late final import2.ViewI55HostBindingFormas0 _compView_0;
  late final import3.I55HostBindingFormas _I55HostBindingFormas_0_5;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import6.ComponentStyles? _componentStyles;
  ViewI56UsaHostBinding0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('i56-usa-host-binding'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i56_usa_host_binding.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewI55HostBindingFormas0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I55HostBindingFormas_0_5 = import3.I55HostBindingFormas();
    this._compView_0.create(this._I55HostBindingFormas_0_5);
    final _anchor_1 = import11.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I56UsaHostBinding1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_1_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i56_usa_host_binding.html:56:71 */;
    this._appEl_1.detectChangesInNestedViews();
    this._compView_0.detectHostChanges(firstCheck);
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I56UsaHostBinding, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I56UsaHostBindingNgFactory = ComponentFactory<import1.I56UsaHostBinding>('i56-usa-host-binding', viewFactory_I56UsaHostBindingHost0);
ComponentFactory<import1.I56UsaHostBinding> get I56UsaHostBindingNgFactory {
  return _I56UsaHostBindingNgFactory;
}

ComponentFactory<import1.I56UsaHostBinding> createI56UsaHostBindingFactory() {
  return ComponentFactory('i56-usa-host-binding', viewFactory_I56UsaHostBindingHost0);
}

class _ViewI56UsaHostBinding1 extends import15.EmbeddedView<import1.I56UsaHostBinding> {
  late final import2.ViewI55HostBindingFormas0 _compView_1;
  late final import3.I55HostBindingFormas _I55HostBindingFormas_1_5;
  _ViewI56UsaHostBinding1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    this._compView_1 = import2.ViewI55HostBindingFormas0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._I55HostBindingFormas_1_5 = import3.I55HostBindingFormas();
    this._compView_1.create(this._I55HostBindingFormas_1_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this._compView_1.detectHostChanges(firstCheck);
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }
}

import15.EmbeddedView<void> viewFactory_I56UsaHostBinding1(import16.RenderView parentView, int parentIndex) {
  return _ViewI56UsaHostBinding1(parentView, parentIndex);
}

final List<Object> styles$I56UsaHostBindingHost = const [];

class _ViewI56UsaHostBindingHost0 extends import17.HostView<import1.I56UsaHostBinding> {
  @override
  void build() {
    this.componentView = ViewI56UsaHostBinding0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I56UsaHostBinding();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.I56UsaHostBinding> viewFactory_I56UsaHostBindingHost0() {
  return _ViewI56UsaHostBindingHost0();
}
