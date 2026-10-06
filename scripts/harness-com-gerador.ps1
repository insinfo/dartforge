# O harness diferencial com o gerador de objetos embutido (a feature
# `llvm-embutido`), para os jobs dos modos de raízes e de exceções do Pesado
# (`nativo-modos`): as raízes por mapas (`--raizes=mapas`, B0 e B1) exigem o
# gerador embutido, porque o Clang não roda o passe dos mapas. O
# `dartforge-diferencial` do artefato `binarios` é compilado sem ele (os
# outros jobs do harness não têm o LLVM de desenvolvimento no PATH); aqui ele
# é recompilado por cima, depois do download e antes do harness (a entrada
# `preparar` da ação `harness`, que já instalou o Rust e o LLVM).
$ErrorActionPreference = 'Stop'
$env:CARGO_PROFILE_RELEASE_LTO = 'false'
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = '16'
cargo build --locked --release -p dartforge-diferencial --features llvm-embutido
if ($LASTEXITCODE -ne 0) { throw 'o harness com o gerador embutido não compilou' }
