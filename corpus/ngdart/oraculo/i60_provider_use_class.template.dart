// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i60_provider_use_class.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i60_provider_use_class.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I60ProviderUseClass = const [];

class ViewI60ProviderUseClass0 extends import0.ComponentView<import1.I60ProviderUseClass> {
  static import2.ComponentStyles? _componentStyles;
  ViewI60ProviderUseClass0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i60-provider-use-class'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i60_provider_use_class.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I60ProviderUseClass, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I60ProviderUseClassNgFactory = ComponentFactory<import1.I60ProviderUseClass>('i60-provider-use-class', viewFactory_I60ProviderUseClassHost0);
ComponentFactory<import1.I60ProviderUseClass> get I60ProviderUseClassNgFactory {
  return _I60ProviderUseClassNgFactory;
}

ComponentFactory<import1.I60ProviderUseClass> createI60ProviderUseClassFactory() {
  return ComponentFactory('i60-provider-use-class', viewFactory_I60ProviderUseClassHost0);
}

final List<Object> styles$I60ProviderUseClassHost = const [];

class _ViewI60ProviderUseClassHost0 extends import9.HostView<import1.I60ProviderUseClass> {
  late import1.I60Impl _I60Base_0_6 = import1.I60Impl();
  late import1.I60OutroImpl _I60Outro_0_7 = import1.I60OutroImpl();
  late import1.I60Terceiro _I60Terceiro_0_8 = import1.I60Terceiro();
  @override
  void build() {
    this.componentView = ViewI60ProviderUseClass0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I60ProviderUseClass();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I60Base)) {
        return this._I60Base_0_6;
      }
      if (identical(token, import1.I60Outro)) {
        return this._I60Outro_0_7;
      }
      if (identical(token, import1.I60Terceiro)) {
        return this._I60Terceiro_0_8;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I60ProviderUseClass> viewFactory_I60ProviderUseClassHost0() {
  return _ViewI60ProviderUseClassHost0();
}
