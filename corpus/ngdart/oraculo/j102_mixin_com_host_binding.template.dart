// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j102_mixin_com_host_binding.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j102_mixin_com_host_binding.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;

final List<Object> styles$J102MixinComHostBinding = const [];

class ViewJ102MixinComHostBinding0 extends import0.ComponentView<import1.J102MixinComHostBinding> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import3.ComponentStyles? _componentStyles;
  ViewJ102MixinComHostBinding0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j102-botao'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j102_mixin_com_host_binding.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'b');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.tabIndex)) /* REF:asset:corpus_ngdart/lib/src/j102_mixin_com_host_binding.dart:626:640 */;
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.tabIndex;
    if (import10.checkBinding(this._expr_0, currVal_0, null, null)) {
      import8.updateAttribute(this.rootElement, 'tabindex', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.papel;
    if (import10.checkBinding(this._expr_1, currVal_1, null, null)) {
      import8.updateAttribute(this.rootElement, 'role', currVal_1);
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.grande;
    if (import10.checkBinding(this._expr_2, currVal_2, null, null)) {
      import8.updateClassBindingNonHtml(this.rootElement, 'grande', currVal_2);
      this._expr_2 = currVal_2;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J102MixinComHostBinding, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J102MixinComHostBindingNgFactory = ComponentFactory<import1.J102MixinComHostBinding>('j102-botao', viewFactory_J102MixinComHostBindingHost0);
ComponentFactory<import1.J102MixinComHostBinding> get J102MixinComHostBindingNgFactory {
  return _J102MixinComHostBindingNgFactory;
}

ComponentFactory<import1.J102MixinComHostBinding> createJ102MixinComHostBindingFactory() {
  return ComponentFactory('j102-botao', viewFactory_J102MixinComHostBindingHost0);
}

final List<Object> styles$J102MixinComHostBindingHost = const [];

class _ViewJ102MixinComHostBindingHost0 extends import12.HostView<import1.J102MixinComHostBinding> {
  @override
  void build() {
    this.componentView = ViewJ102MixinComHostBinding0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J102MixinComHostBinding();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import12.HostView<import1.J102MixinComHostBinding> viewFactory_J102MixinComHostBindingHost0() {
  return _ViewJ102MixinComHostBindingHost0();
}

final List<Object> styles$J102Usa = const [];

class ViewJ102Usa0 extends import0.ComponentView<import1.J102Usa> {
  late final ViewJ102MixinComHostBinding0 _compView_0;
  late final import1.J102MixinComHostBinding _J102MixinComHostBinding_0_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ102Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j102-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j102_mixin_com_host_binding.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ102MixinComHostBinding0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    _el_0.tabIndex = 3;
    this._J102MixinComHostBinding_0_5 = import1.J102MixinComHostBinding();
    this._compView_0.create(this._J102MixinComHostBinding_0_5);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J102MixinComHostBinding_0_5, 'tabindex', '3');
      }
      this._J102MixinComHostBinding_0_5.tabindex = '3' /* REF:asset:corpus_ngdart/lib/src/j102_mixin_com_host_binding.dart:816:828 */;
    }
    this._compView_0.detectHostChanges(firstCheck);
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J102Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J102UsaNgFactory = ComponentFactory<import1.J102Usa>('j102-usa', viewFactory_J102UsaHost0);
ComponentFactory<import1.J102Usa> get J102UsaNgFactory {
  return _J102UsaNgFactory;
}

ComponentFactory<import1.J102Usa> createJ102UsaFactory() {
  return ComponentFactory('j102-usa', viewFactory_J102UsaHost0);
}

final List<Object> styles$J102UsaHost = const [];

class _ViewJ102UsaHost0 extends import12.HostView<import1.J102Usa> {
  @override
  void build() {
    this.componentView = ViewJ102Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J102Usa();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J102Usa> viewFactory_J102UsaHost0() {
  return _ViewJ102UsaHost0();
}
