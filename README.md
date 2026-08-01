# DuckDNS Updater ⚡

> ⚡ **Aplicação Vibecodada por Leandro Pinheiro**

DuckDNS Updater é um aplicativo leve, rápido e nativo escrito em Rust com interface **Windows 11 Fluent Dark (WinUI 3)**, projetado para manter seus IPs públicos (IPv4 e IPv6) automaticamente atualizados no serviço [DuckDNS](https://www.duckdns.org/).

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

- 🎨 **Interface Windows 11 WinUI 3 Dark**: Design limpo com cartões arredondados, fonte nativa Segoe UI e acentuação moderna.
- 🚀 **Iniciar com o Windows**: Opção integrada para registrar a inicialização automática no Windows.
- 📌 **System Tray & Start Minimized**: Minimiza para a bandeja do sistema e pode iniciar diretamente oculta no boot.
- 🌐 **Multi-Domínio**: Suporte para atualizar múltiplos domínios DuckDNS simultaneamente (separados por vírgula).
- 🧠 **Smart IP Change Detection**: Atualiza a API do DuckDNS apenas quando o IP público realmente muda.
- 🔄 **Retry com Backoff Exponencial**: Em caso de falha de conexão, tenta novamente com delays inteligentes (5s, 15s, 45s).
- 📊 **Histórico de Atualizações**: Registra o log das últimas atualizações com opção de **Exportar para CSV**.
- ⏳ **Countdown ao Vivo**: Temporizador regressivo mostrando exatamente quanto tempo falta para a próxima verificação.
- 🛡️ **Validação de Campos**: Indicadores visuais dinâmicos para campos incorretos ou não preenchidos.
- ⌨️ **Atalhos de Teclado**: `Ctrl+S` (Salvar), `Ctrl+U` (Atualizar), `Escape` (Ocultar na tray).
- 🔔 **Notificações Desktop Nativa**: Alertas discretos no sistema operacional quando o IP muda.
- ⚙️ **CI/CD Automático**: Compilação e publicação de executáveis automatizada via **GitHub Actions**.

---

## 🛠️ Como Compilar e Rodar

### Compilação Local

1. Instale o Rust ([rustup.rs](https://rustup.rs/)).
2. Clone o repositório e execute:

```bash
cargo build --release
```

O executável otimizado estará em `target/release/duckdns-updater.exe`.

---

## 👨‍💻 Créditos

- **Desenvolvedor:** Leandro Pinheiro
- **Vibecodado com:** Rust, egui e IA
- **Licença:** [MIT](LICENSE)
