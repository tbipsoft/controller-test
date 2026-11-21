#!/bin/bash
#
# Controller Test -- launcher do menu "Tools" do dArkOS para o app rg52-pad.
# Mostra na tela o desenho do controle e o log de tudo que botoes, gatilhos e analogicos
# enviam. PARA SAIR: segure START + HOME por 1 segundo.
#
# O app so le os controles (nao precisa de root). Onde colocar: este .sh na pasta dos
# scripts do menu Tools e o binario `rg52-pad` numa destas opcoes:
#   <pasta deste script>/rg52-pad/rg52-pad   (pasta "rg52-pad" ao lado do .sh)
#   <pasta deste script>/rg52-pad            (binario ao lado do .sh)
#   /opt/rg52-pad/rg52-pad
# Variaveis opcionais:
#   EXIT_COMBO=start+select   outra combinacao de saida (aceita botoes crus: start+b12)
#   MAP=back=b13,home=b12     atribui botoes crus (sem nome no SDL) ao desenho
#   LAYOUT=xbox               posicao dos botoes de face (nintendo e o padrao)
#   ACCENT=teal               cor de destaque (amber, teal, violet, coral ou #RRGGBB)

APP=rg52-pad
LOG=/tmp/rg52-pad.log
RUN=/tmp/rg52-pad.run
TTY=${TTY:-/dev/tty1}

HERE="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"

say() { printf '\033c%s\n' "$1" > "$TTY" 2>/dev/null; sleep "${2:-5}"; }

find_bin() {
  local c
  for c in "$HERE/$APP/$APP" "$HERE/$APP" "/opt/$APP/$APP"; do
    [ -f "$c" ] && { echo "$c"; return 0; }
  done
  return 1
}

cleanup() {
  rm -f "$RUN"
  printf '\033c' > "$TTY" 2>/dev/null
}

sudo chmod 666 "$TTY" 2>/dev/null
printf '\033c\e[?25l' > "$TTY" 2>/dev/null

BIN=$(find_bin) || {
  say "rg52-pad nao encontrado.

Coloque o binario em uma destas opcoes:
  $HERE/$APP/$APP
  $HERE/$APP
  /opt/$APP/$APP" 8
  cleanup
  exit 1
}

# Copia para /tmp: funciona mesmo com o binario numa particao FAT/noexec.
if ! cp -f "$BIN" "$RUN" || ! chmod +x "$RUN"; then
  say "Nao consegui preparar $RUN (disco cheio ou /tmp somente leitura?)" 6
  cleanup
  exit 1
fi

# Sem gptokeyb de proposito: ele converteria os botoes em teclas e atrapalharia o teste.
# Se um gptokeyb de outro menu ficou rodando, encerra para nao disputar os eventos.
pgrep -f gptokeyb | sudo xargs -r kill -9 2>/dev/null

export SDL_GAMECONTROLLERCONFIG_FILE="/opt/inttools/gamecontrollerdb.txt"
export XDG_RUNTIME_DIR="/run/user/$UID/"

ARGS=(--fullscreen)
[ -n "$EXIT_COMBO" ] && ARGS+=(--exit "$EXIT_COMBO")
[ -n "$MAP" ] && ARGS+=(--map "$MAP")
[ -n "$LAYOUT" ] && ARGS+=(--layout "$LAYOUT")
[ -n "$ACCENT" ] && ARGS+=(--accent "$ACCENT")

env \
  SDL_GAMECONTROLLERCONFIG_FILE="$SDL_GAMECONTROLLERCONFIG_FILE" \
  XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
  ${SDL_VIDEODRIVER:+SDL_VIDEODRIVER="$SDL_VIDEODRIVER"} \
  "$RUN" "${ARGS[@]}" > "$LOG" 2>&1
STATUS=$?

if [ "$STATUS" -ne 0 ]; then
  say "rg52-pad terminou com erro ($STATUS).

$(tail -n 12 "$LOG" 2>/dev/null)

Log completo: $LOG" 10
fi

cleanup
exit "$STATUS"
