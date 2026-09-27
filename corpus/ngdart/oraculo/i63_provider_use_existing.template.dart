// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i63_provider_use_existing.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i63_provider_use_existing.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I63ProviderUseExisting = const [];

class ViewI63ProviderUseExisting0 extends import0.ComponentView<import1.I63ProviderUseExisting> {
  static import2.ComponentStyles? _componentStyles;
  ViewI63ProviderUseExisting0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i63-provider-use-existing'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i63_provider_use_existing.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I63ProviderUseExisting, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I63ProviderUseExistingNgFactory = ComponentFactory<import1.I63ProviderUseExisting>('i63-provider-use-existing', viewFactory_I63ProviderUseExistingHost0);
ComponentFactory<import1.I63ProviderUseExisting> get I63ProviderUseExistingNgFactory {
  return _I63ProviderUseExistingNgFactory;
}

ComponentFactory<import1.I63ProviderUseExisting> createI63ProviderUseExistingFactory() {
  return ComponentFactory('i63-provider-use-existing', viewFactory_I63ProviderUseExistingHost0);
}

final List<Object> styles$I63ProviderUseExistingHost = const [];

class _ViewI63ProviderUseExistingHost0 extends import9.HostView<import1.I63ProviderUseExisting> {
  late import1.I63Servico _I63Servico_0_6 = import1.I63Servico();
  @override
  void build() {
    this.componentView = ViewI63ProviderUseExisting0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I63ProviderUseExisting();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I63Pai)) {
        return this.component;
      }
      if (((identical(token, import1.I63Servico) || identical(token, import1.I63Base)) || identical(token, import1.I63Leitor))) {
        return this._I63Servico_0_6;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I63ProviderUseExisting> viewFactory_I63ProviderUseExistingHost0() {
  return _ViewI63ProviderUseExistingHost0();
}
