// LLJIT's default COFF layer enables these flags. Its C API factory for custom
// memory managers does not expose them, so retain the same symbol ownership
// policy when installing DartForge's bounded allocator. No heap ownership is
// transferred across the LLVM-C.dll boundary.
#include "llvm/ExecutionEngine/Orc/RTDyldObjectLinkingLayer.h"

extern "C" void dartforge_configure_coff_layer(void *opaque) {
    auto *base = reinterpret_cast<llvm::orc::ObjectLayer *>(opaque);
    auto *layer = static_cast<llvm::orc::RTDyldObjectLinkingLayer *>(base);
    layer->setOverrideObjectFlagsWithResponsibilityFlags(true);
    layer->setAutoClaimResponsibilityForObjectSymbols(true);
}
