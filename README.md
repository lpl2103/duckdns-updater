# DuckDNS Updater ⚡

> ⚡ **Aplicação Vibecodada por Leandro Pinheiro**

DuckDNS Updater é um aplicativo leve, rápido e nativo escrito em **Rust + egui** com interface **Windows 11 Fluent Dark (WinUI 3)**, projetado para manter seus IPs públicos (IPv4 e IPv6) automaticamente atualizados no serviço [DuckDNS](https://www.duckdns.org/), com **Auto-Atualização automática via GitHub**, suporte a múltiplos domínios, inicialização com o Windows e operação silenciosa na bandeja do sistema (System Tray).

---

## 🔓 Licença & Liberdade de Uso

Este projeto é **100% livre e open-source** sob a **Licença MIT**.

Você tem total liberdade para:
- 💡 **Usar** para qualquer finalidade (pessoal ou comercial)
- ✏️ **Editar e modificar** o código da forma que quiser
- 📢 **Compartilhar e redistribuir** livremente
- 🏗️ **Fazer fork ou derivar** novos projetos

Sinta-se à vontade para estudar, clonar, melhorar e fazer o que bem entender com o código!

---

## ✨ Recursos

- 🔄 **Auto-Atualização Integrada (GitHub Releases)**:
  - Checagem automática assíncrona de novas versões no repositório GitHub ao iniciar.
  - Notificação modal amigável com notas da versão e botão **"Atualizar Agora"**.
  - **Substituição Quente no Windows (Hot-Swap)**: Baixa a nova versão em streaming, efetua o swap seguro dos executáveis e reinicia o processo automaticamente com autolimpeza de arquivos residuais (`--cleanup-old`).
  - Verificação e atualização sob demanda na janela **Sobre e Atualizações**.
- 🌐 **Fallback Multi-Provedor de IP Público com Validação Estrita**:
  - Detecção resiliente de IPv4 e IPv6 com redundância entre múltiplos provedores globais confiáveis (`api.ipify.org`, `icanhazip.com`, `ident.me`, `ifconfig.me`, `checkip.amazonaws.com`).
  - Validação rigorosa de sintaxe com o parser `std::net::Ipv4Addr` e `Ipv6Addr`, rejeitando páginas de erro de provedores e garantindo que apenas IPs válidos sejam enviados para a API do DuckDNS.
- 🔍 **Diagnóstico & Propagação DNS via DoH (DNS-over-HTTPS)**:
  - Botão integrado **"Testar Resolução DNS"** para verificar em tempo real se os subdomínios DuckDNS já propagaram globalmente.
  - Consulta segura via DoH (Cloudflare e Google) imune a bloqueios de firewall UDP porta 53 e caches desatualizados de roteadores ou ISPs locais.
- 🔔 **Notificações Externas via Webhook (Discord & Telegram)**:
  - Envio automático de alertas ricos para canais do **Discord** (Embed estilizado) e bots do **Telegram** (Markdown).
  - Botão interativo para **Testar Webhook** antes de salvar.
  - Opção para **Notificar apenas quando o IP mudar** ou em todas as atualizações.
- 🔒 **Proteção de Credenciais com Windows DPAPI**:
  - O Token secreto do DuckDNS, a URL do Webhook do Discord e o Token do Bot do Telegram são criptografados com a API nativa do Windows (`CryptProtectData`) antes de serem salvos em disco.
- 🖥️ **Modo Servidor / Daemon Headless (`--daemon`)**:
  - Suporte completo para execução sem interface gráfica em servidores Windows, serviços do sistema (`nssm`) ou tarefas agendadas.
  - Inicialização automática com `duckdns-updater.exe --daemon` ou `--help`.
- 📋 **Facilidades de 1 Clique (Copiar IP)**:
  - Botões rápidos `📋` para copiar o IPv4 e IPv6 atuais diretamente para a área de transferência com confirmação instantânea.
- 🎨 **Interface Windows 11 WinUI 3 Dark**: Design limpo com cartões arredondados, fonte nativa Segoe UI, paleta moderna e feedback visual de sucesso (*glow flash*).
- 🚀 **Iniciar com o Windows**: Opção integrada para registrar a inicialização automática no Registro do Windows (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
- 📌 **System Tray & Start Minimized**: Minimiza para a bandeja do sistema ao fechar no `[X]` ou minimizar, com suporte a iniciar diretamente oculto no boot do computador.
- 🌐 **Multi-Domínio**: Suporte para atualizar múltiplos domínios DuckDNS simultaneamente (separados por vírgula).
- 🧠 **Smart IP Change Detection**: Atualiza a API do DuckDNS apenas quando o IP público realmente muda, economizando requisições.
- 🔄 **Retry com Backoff Exponencial**: Em caso de oscilações ou falha na conexão, realiza novas tentativas com intervalos inteligentes (5s, 15s, 45s).
- 📊 **Histórico de Atualizações & Exportação CSV**: Registra o log das últimas atualizações em formato cronológico com botão de 1 clique para exportar diretamente para CSV.
- ⏳ **Countdown ao Vivo**: Temporizador regressivo e barra de progresso mostrando exatamente quanto tempo falta para a próxima verificação programada.
- 🛡️ **Validação Dinâmica de Campos**: Indicadores visuais dinâmicos para campos incorretos, incompletos ou em branco.
- ⌨️ **Atalhos Rápidos de Teclado**:
  - `Ctrl + S`: Salvar configurações instantaneamente.
  - `Ctrl + U`: Forçar atualização imediata do IP.
  - `Escape`: Ocultar aplicativo na bandeja do sistema.
- 🔔 **Notificações Desktop Nativas**: Alertas discretos no sistema operacional quando o IP público for alterado enquanto o app estiver minimizado.
- 🖼️ **Ícone e Metadados Nativos do Windows**:
  - Binário com ícone embutido em alta resolução (resoluções de 16x16 até 256x256) exibido no Windows Explorer, barra de tarefas e Alt+Tab.
  - Tabela de metadados completa (`VERSIONINFO`): Nome do Produto, Descrição, Versão, Copyright e Desenvolvedor.
- ⚙️ **CI/CD Automático**: Compilação e publicação de executáveis automatizada via **GitHub Actions**.

---

## 💻 Modos de Execução (CLI & Daemon)

O executável detecta o terminal automaticamente e oferece modos de linha de comando:

```powershell
# Execução normal com interface gráfica WinUI 3:
.\duckdns-updater.exe

# Iniciar minimizado na bandeja do sistema:
.\duckdns-updater.exe --minimized

# Executar como daemon/servidor sem GUI (ideal para VPS, servidores e serviços):
.\duckdns-updater.exe --daemon

# Exibir opções e versão:
.\duckdns-updater.exe --help
```

---

## 🛠️ Como Compilar e Rodar

### Pré-requisitos:
- [Rust](https://rustup.rs/) (versão Stable recomendada).

### Compilação Local:
```bash
cargo build --release
```

O executável otimizado estará localizado em:
```text
target/release/duckdns-updater.exe
```

---

## 📂 Arquivos do Sistema

- **Configurações JSON**: `%APPDATA%\duckdns-updater\duckdns_config.json`
- **Histórico JSON**: `%APPDATA%\duckdns-updater\duckdns_history.json`

---

## 👨‍💻 Créditos

- **Desenvolvedor:** Leandro Pinheiro
- **Vibecodado com:** Rust, egui e IA
- **Licença:** [MIT](LICENSE)
