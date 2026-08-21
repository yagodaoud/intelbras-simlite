# intelbras-simlite

Viewer **leve e open source** de CFTV Intelbras (DVR/NVR), feito em **Rust + Slint**.

Objetivo: live e reprodução do HD do DVR via **RTSP**, sem depender do SIM Next (pesado) e **sem baixar** arquivos `.dav` pro PC.

Licença: [MIT](LICENSE) (se o arquivo ainda não existir no repo, o `Cargo.toml` já declara MIT).

---

## O que faz

- **Ao vivo** — mosaico automático (1 / 2 / 4 / 6) conforme as câmeras selecionadas
- **Reprodução** — gravação do HD do DVR por RTSP (timeline do dia, seek, 0.5x / 1x / 2x / 4x)
- **Modo Qualidade / Performance** no live (opção salva localmente)
- Zoom com scroll no ponto do cursor + arrastar (mãozinha) para navegar
- Arrastar tiles no mosaico para trocar posição (preferência salva)
- Senha no **Credential Manager** do Windows — nunca em `config.json` nem em logs

---

## Requisitos

| Item | Detalhe |
|------|---------|
| SO | Windows 10/11 (x64) — foco atual |
| [Rust](https://rustup.rs/) | toolchain estável (`rustup` / `cargo`) |
| [FFmpeg](https://ffmpeg.org/) | no `PATH` (`ffmpeg -version` deve funcionar) |
| DVR/NVR Intelbras | RTSP acessível na rede (porta típica `554`) |

Conta no DVR: preferir usuário **viewer** (não `admin`), com permissão de live e playback.

---

## Build

No PowerShell, na pasta do projeto:

```powershell
# Dependências (só na primeira vez)
rustup update
# Confirme o FFmpeg:
ffmpeg -version

# Build de desenvolvimento
cargo build

# Build de produção (recomendado — mais rápido e leve)
cargo build --release
```

Binário release:

```text
.\target\release\simlite.exe
```

Testes:

```powershell
cargo test
```

---

## Executar

```powershell
cargo run --release
# ou
.\target\release\simlite.exe
```

Na primeira abertura:

1. Informe IP/host do DVR, usuário e senha  
2. Portas RTSP (ex.: `554`) e HTTP (só se for usar CGI; a reprodução usa RTSP)  
3. Quantidade de canais  
4. **Salvar e conectar** — a senha vai para o Credential Manager

Configuração (sem senha) fica em:

```text
%AppData%\simlite\SIM Lite\config\config.json
```

---

## Uso rápido

### Ao vivo

- Marque câmeras na sidebar — o grid ajusta sozinho  
- **Qualidade** = stream principal, mais FPS/resolução  
- **Performance** = substream, mais leve no CPU  
- Scroll no vídeo = zoom; com zoom, arraste com a mãozinha  
- Arraste um tile sobre outro para reordenar  
- Duplo clique no tile remove; a próxima câmera clicada na lista preenche o slot

### Reprodução

- Selecione uma ou mais câmeras (grid como no ao vivo)  
- Escolha a data no calendário  
- **Abrir reprodução**  
- Timeline: horas do dia, barra verde, playhead; clique para seek  
- Scroll na timeline = zoom da escala; botões `−` / `+` também  
- Velocidades: o DVR não acelera RTSP de verdade — em 2x/4x o app avança com seek-jump

---

## Segurança (resumo)

- Senha **nunca** entra em `config.json`, logs, `Display`/`Debug` ou mensagens de erro  
- Host/usuário validados (sem injeção em URL RTSP/CGI)  
- RTSP sempre com transporte **TCP**  
- Playback vem do HD do DVR (RTSP), não download local de `.dav`

---

## Stack

- **Rust** — domínio, RTSP, player (FFmpeg subprocess → RGB)  
- **Slint** — UI  
- **FFmpeg** — decode (hwaccel `d3d11va` quando disponível)

---

## Limitações conhecidas

- Foco em Windows; outros SOs não estão priorizados  
- CGI de busca de gravações pode falhar se a porta web do DVR não for alcançável; a timeline assume presença contínua no dia quando não há lista CGI  
- Modelos/firmwares Intelbras variam — se o RTSP live ou playback falhar, confira usuário, porta e firewall

---

## Contribuir

Issues e PRs são bem-vindos. Mantenha o espírito do projeto: leve, seguro com credenciais, e sem baixar o arquivo de gravação pro disco local sem necessidade.

```powershell
git clone https://github.com/yagodaoud/intelbras-simlite.git
cd intelbras-simlite
cargo test
cargo build --release
```

---

## Licença

MIT — veja o `Cargo.toml` e, se presente, o arquivo `LICENSE`.
