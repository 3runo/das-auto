# Das Auto

Gestão de carros, documentos e despesas — local-first, Rust + Dioxus, backend Supabase.

> Plano do projeto, progresso e decisões: [`plan/`](plan/).

## Workspace

| Crate | Papel |
|---|---|
| `crates/core` | Domínio (dinheiro, placa, storage, regras) — pura, sem I/O, sem UI |
| `crates/net` | Cliente Supabase (auth, CRUD, sync) — mockável |
| `app` | UI Dioxus (mobile/web): shell de 5 abas, captura, PDF |

## Desenvolvimento

```sh
# gates (mesmos do CI)
cargo fmt --all -- --check
cargo clippy -p dasauto-core -p dasauto-net --all-targets -- -D warnings
cargo test -p dasauto-core -p dasauto-net
cargo check -p dasauto-core -p dasauto-net --target wasm32-unknown-unknown

# web (dev server / build)
cd app && dx serve --web
cd app && dx build --web   # → app/target/dx/das-auto/debug/web/public

# iOS (simulador) — PITFALL: NÃO usar --device (pede identidade Apple real);
# sem --device builda para o simulador ativo e assina ad-hoc.
cd app && dx build --ios
xcrun simctl install booted app/target/dx/das-auto/debug/ios/*.app
xcrun simctl launch booted com.tacto.dasauto
```

## CI

`.github/workflows/ci.yml`: fmt, clippy (`-D warnings`), testes host de `core`/`net`,
`cargo check` para `wasm32-unknown-unknown`, e smoke `dx build --web`.
