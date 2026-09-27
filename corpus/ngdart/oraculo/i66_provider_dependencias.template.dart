// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i66_provider_dependencias.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i66_provider_dependencias.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I66ProviderDependencias = const [];

class ViewI66ProviderDependencias0 extends import0.ComponentView<import1.I66ProviderDependencias> {
  static import2.ComponentStyles? _componentStyles;
  ViewI66ProviderDependencias0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i66-provider-dependencias'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i66_provider_dependencias.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I66ProviderDependencias, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I66ProviderDependenciasNgFactory = ComponentFactory<import1.I66ProviderDependencias>('i66-provider-dependencias', viewFactory_I66ProviderDependenciasHost0);
ComponentFactory<import1.I66ProviderDependencias> get I66ProviderDependenciasNgFactory {
  return _I66ProviderDependenciasNgFactory;
}

ComponentFactory<import1.I66ProviderDependencias> createI66ProviderDependenciasFactory() {
  return ComponentFactory('i66-provider-dependencias', viewFactory_I66ProviderDependenciasHost0);
}

final List<Object> styles$I66ProviderDependenciasHost = const [];

class _ViewI66ProviderDependenciasHost0 extends import9.HostView<import1.I66ProviderDependencias> {
  late import1.I66Cache _I66Cache_0_8 = import1.I66Cache(this._I66Repo_0_6);
  late import1.I66Solto _I66Solto_0_9 = import1.I66Solto();
  late final import1.I66Api _I66Api_0_5;
  late final import1.I66Repo _I66Repo_0_6;
  @override
  void build() {
    this.componentView = ViewI66ProviderDependencias0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._I66Api_0_5 = import1.I66Api();
    this._I66Repo_0_6 = import1.I66Repo(this._I66Api_0_5);
    this.component = import1.I66ProviderDependencias(this._I66Repo_0_6);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I66Api)) {
        return this._I66Api_0_5;
      }
      if (identical(token, import1.I66Repo)) {
        return this._I66Repo_0_6;
      }
      if (identical(token, import1.I66Cache)) {
        return this._I66Cache_0_8;
      }
      if (identical(token, import1.I66Solto)) {
        return this._I66Solto_0_9;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I66ProviderDependencias> viewFactory_I66ProviderDependenciasHost0() {
  return _ViewI66ProviderDependenciasHost0();
}
