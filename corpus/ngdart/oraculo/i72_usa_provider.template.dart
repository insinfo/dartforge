// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i72_usa_provider.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i72_usa_provider.dart' as import1;
import 'i60_provider_use_class.dart' as import2;
import 'i60_provider_use_class.template.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I72UsaProvider = const [];

class ViewI72UsaProvider0 extends import0.ComponentView<import1.I72UsaProvider> {
  late import2.I60Impl _I60Base_0_6 = import2.I60Impl();
  late import2.I60OutroImpl _I60Outro_0_7 = import2.I60OutroImpl();
  late import2.I60Terceiro _I60Terceiro_0_8 = import2.I60Terceiro();
  late final import3.ViewI60ProviderUseClass0 _compView_0;
  late final import2.I60ProviderUseClass _I60ProviderUseClass_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewI72UsaProvider0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i72-usa-provider'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i72_usa_provider.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import3.ViewI60ProviderUseClass0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I60ProviderUseClass_0_5 = import2.I60ProviderUseClass();
    this._compView_0.create(this._I60ProviderUseClass_0_5);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import2.I60Base)) {
        return this._I60Base_0_6;
      }
      if (identical(token, import2.I60Outro)) {
        return this._I60Outro_0_7;
      }
      if (identical(token, import2.I60Terceiro)) {
        return this._I60Terceiro_0_8;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I72UsaProvider, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I72UsaProviderNgFactory = ComponentFactory<import1.I72UsaProvider>('i72-usa-provider', viewFactory_I72UsaProviderHost0);
ComponentFactory<import1.I72UsaProvider> get I72UsaProviderNgFactory {
  return _I72UsaProviderNgFactory;
}

ComponentFactory<import1.I72UsaProvider> createI72UsaProviderFactory() {
  return ComponentFactory('i72-usa-provider', viewFactory_I72UsaProviderHost0);
}

final List<Object> styles$I72UsaProviderHost = const [];

class _ViewI72UsaProviderHost0 extends import10.HostView<import1.I72UsaProvider> {
  @override
  void build() {
    this.componentView = ViewI72UsaProvider0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I72UsaProvider();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I72UsaProvider> viewFactory_I72UsaProviderHost0() {
  return _ViewI72UsaProviderHost0();
}
