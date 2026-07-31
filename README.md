# DuckDNS Updater

DuckDNS Updater é uma aplicação leve, nativa e multiplataforma escrita em Rust, projetada para manter o seu IP público atualizado no serviço [DuckDNS](https://www.duckdns.org/). 

Com uma interface gráfica elegante (usando `egui` e `eframe`) e integração com a bandeja do sistema (System Tray), o app roda de forma silenciosa e eficiente, garantindo que seu domínio aponte sempre para o IP correto (suporta IPv4 e IPv6).

## Recursos

- **Interface Gráfica Simples**: Configuração fácil do domínio, token e intervalo de atualização.
- **Integração com System Tray (Bandeja do Sistema)**: O aplicativo pode ser minimizado para a bandeja, operando em segundo plano.
- **Menu de Contexto no Tray**: Acesso rápido para abrir configurações, forçar atualização ou sair, tudo via chamadas nativas, funcionando de forma 100% independente do event loop principal.
- **Notificações Desktop**: Receba notificações nativas ao atualizar o IP (somente quando o aplicativo estiver minimizado, para não ser intrusivo).
- **Atualização Automática**: Atualiza o IP periodicamente através de uma thread dedicada em segundo plano.
- **Leve e Eficiente**: Construído em Rust, consome pouquíssima memória e CPU.

## Como Compilar e Rodar

### Pré-requisitos

1. **Rust**: Certifique-se de ter a toolchain do Rust instalada ([rustup](https://rustup.rs/)).
2. Para compilação no **Windows**, certifique-se de ter o ambiente de compilação C++ (MSVC ou MinGW) configurado.

### Build (Release)

Para gerar um binário otimizado para uso diário, abra o terminal na raiz do repositório e execute:

```bash
cargo build --release
```

O executável pronto para uso estará em `target/release/duckdns-updater.exe`.

### Executando

Basta rodar o executável. Na primeira vez, ele exibirá a tela principal. Preencha seu **Domínio**, **Token DuckDNS**, e **Intervalo** e clique em "Salvar Settings". 
Você pode ocultar o app no Tray, e ele continuará monitorando e atualizando o IP silenciosamente.

## Tecnologias Utilizadas

- **[eframe / egui](https://github.com/emilk/egui)**: Interface gráfica imediata (Immediate Mode GUI).
- **[tray-icon](https://github.com/tauri-apps/tray-icon)**: Suporte multiplataforma para o System Tray.
- **[ureq](https://github.com/algesten/ureq)**: Cliente HTTP síncrono ultra leve para a comunicação com a API do DuckDNS e verificação de IP.
- **[notify-rust](https://github.com/maciejhirsz/notify-rust)**: Notificações de desktop nativas.
- **winapi**: Chamadas nativas de sistema para garantir a reatividade do Tray no Windows mesmo quando a janela principal está suspensa.
