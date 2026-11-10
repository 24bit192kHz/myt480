# ~/.zshrc — native zsh, no framework (2026-09-28; was oh-my-zsh + starship,
# 333 ms start). Old file: ~/.local/share/backups/dotfiles/2026-09-28/zshrc.pre-native
# Parts: ~/.config/zsh/{fzf.zsh,functions/}. Theme colours follow
# ~/.config/theme/palette.conf (greys; red only for errors).

ZDOT=~/.config/zsh
ZCACHE=${XDG_CACHE_HOME:-$HOME/.cache}/zsh

# --- env / path -------------------------------------------------------------
typeset -U path fpath
# nvm is lazy (below); the newest installed node is on PATH directly
_node=(~/.nvm/versions/node/*/bin(N/On[1]))
path=(~/.local/bin ~/.npm-global/bin ~/.opencode/bin $_node $path)
unset _node
fpath=($ZDOT/functions $fpath)
autoload -Uz scp
export EDITOR=vim VISUAL=vim PAGER=less LESS='-R --mouse' MANPAGER='less -R'
export NVM_DIR=~/.nvm
# ls/eza/completion colours in palette.conf greys (was unset, so ls used its
# built-in colours, where "blue" dirs show as C4 red); red only for broken links
export LS_COLORS='rs=0:di=1;38;2;240;240;240:ln=3;38;2;184;184;184:or=38;2;204;51;51:mi=38;2;204;51;51:ex=1;38;2;207;207;207:pi=38;2;144;144;144:so=38;2;144;144;144:do=38;2;144;144;144:bd=38;2;144;144;144:cd=38;2;144;144;144:su=4;38;2;228;228;228:sg=4;38;2;228;228;228:tw=1;4;38;2;240;240;240:ow=1;4;38;2;240;240;240:st=1;38;2;240;240;240:*.tar=38;2;168;168;168:*.gz=38;2;168;168;168:*.xz=38;2;168;168;168:*.zst=38;2;168;168;168:*.zip=38;2;168;168;168:*.7z=38;2;168;168;168:*.rar=38;2;168;168;168:*.bz2=38;2;168;168;168:*.iso=38;2;168;168;168:*.img=38;2;168;168;168:*.rom=38;2;168;168;168:*.bin=38;2;168;168;168:*.pkg.tar.zst=38;2;168;168;168:*.png=38;2;200;200;200:*.jpg=38;2;200;200;200:*.jpeg=38;2;200;200;200:*.gif=38;2;200;200;200:*.webp=38;2;200;200;200:*.svg=38;2;200;200;200:*.mp4=38;2;200;200;200:*.mkv=38;2;200;200;200:*.webm=38;2;200;200;200:*.mp3=38;2;200;200;200:*.flac=38;2;200;200;200:*.pdf=38;2;200;200;200:*.bak=38;2;110;110;110:*.old=38;2;110;110;110:*.tmp=38;2;110;110;110:*.swp=38;2;110;110;110:*~=38;2;110;110;110:*.log=38;2;144;144;144'

# --- history ----------------------------------------------------------------
HISTFILE=~/.zsh_history HISTSIZE=50000 SAVEHIST=50000
setopt extended_history hist_expire_dups_first hist_ignore_dups \
       hist_ignore_space hist_verify share_history hist_reduce_blanks

# --- options ----------------------------------------------------------------
setopt auto_cd auto_pushd pushd_ignore_dups pushd_minus interactive_comments \
       extended_glob no_beep complete_in_word always_to_end
# words stop at punctuation (/ - . _ = etc.), so Ctrl+Left/Right, Ctrl+Backspace
# and Ctrl+Right on a suggestion go one piece at a time
WORDCHARS=''

# --- completion: dump rebuilt at most once a day, otherwise -C (no audit) ---
autoload -Uz compinit
if [[ -n $ZCACHE/zcompdump(#qN.mh-24) ]]; then
  compinit -C -d $ZCACHE/zcompdump
else
  mkdir -p $ZCACHE; compinit -d $ZCACHE/zcompdump
  { zcompile $ZCACHE/zcompdump } &!
fi
_comp_options+=(globdots)
zstyle ':completion:*' menu select
zstyle ':completion:*' matcher-list 'm:{a-zA-Z}={A-Za-z}' 'r:|=*' 'l:|=* r:|=*'
# file colours as in ls; ma = the selected entry (SELECTED bg, FG_BRIGHT)
zstyle ':completion:*' list-colors ${(s.:.)LS_COLORS} 'ma=48;2;42;42;42;1;38;2;240;240;240'
zstyle ':completion:*' group-name ''
zstyle ':completion:*' list-dirs-first true
zstyle ':completion:*' use-cache on
zstyle ':completion:*' cache-path $ZCACHE/compcache
zstyle ':completion:*:descriptions' format '%F{244}-- %d --%f'
zstyle ':completion:*:messages' format '%F{244}-- %d --%f'
zstyle ':completion:*:warnings' format '%F{167}-- no matches --%f'
zstyle ':completion:*:*:kill:*:processes' list-colors '=(#b) #([0-9]#)*=0=38;5;250'

# --- keys (emacs mode + the usual terminal escapes) -------------------------
bindkey -e
autoload -Uz up-line-or-beginning-search down-line-or-beginning-search
zle -N up-line-or-beginning-search; zle -N down-line-or-beginning-search
bindkey '^[[A' up-line-or-beginning-search   '^[OA' up-line-or-beginning-search
bindkey '^[[B' down-line-or-beginning-search '^[OB' down-line-or-beginning-search
bindkey '^[[H' beginning-of-line '^[[1~' beginning-of-line '^[OH' beginning-of-line
bindkey '^[[F' end-of-line       '^[[4~' end-of-line       '^[OF' end-of-line
# st sends ^H for Ctrl+Backspace (plain Backspace is ^?)
# st sends ^[[P for Delete, ^[[M for Ctrl+Delete (^[[3~ / ^[[3;5~ only in keypad mode)
bindkey '^[[P' delete-char '^[[M' kill-word
bindkey '^[[3~' delete-char '^?' backward-delete-char '^H' backward-kill-word
bindkey '^[[1;5C' forward-word '^[[1;5D' backward-word '^[[3;5~' kill-word
bindkey '^[[Z' reverse-menu-complete
# Esc Esc: prefix (or strip) sudo on the current / last command
_sudo_toggle() {
  [[ -z $BUFFER ]] && BUFFER=$history[$((HISTCMD-1))]
  if [[ $BUFFER == sudo\ * ]]; then BUFFER=${BUFFER#sudo }; else BUFFER="sudo $BUFFER"; fi
  CURSOR=$#BUFFER
}
zle -N _sudo_toggle; bindkey '\e\e' _sudo_toggle

# --- prompt: 󰣇 dir branch ❯  (grey; ❯ red after a failure; took Ns if >3 s) --
autoload -Uz add-zsh-hook
# git branch/action read straight from .git, no git process (vcs_info ran git
# 2-3 times per prompt: ~11 ms in a repo, ~4 ms outside; this is ~0.1 ms)
_p_git() {
  _p_branch=
  local d=$PWD g head a
  while true; do
    if [[ -d $d/.git ]]; then g=$d/.git; break
    elif [[ -f $d/.git ]]; then   # worktree/submodule: "gitdir: <path>"
      g=$(<$d/.git); g=${g#gitdir: }; [[ $g == /* ]] || g=$d/$g; break
    fi
    [[ $d == / ]] && return
    d=${d:h}
  done
  [[ -r $g/HEAD ]] || return
  head=$(<$g/HEAD)
  if [[ $head == 'ref: refs/heads/'* ]]; then head=${head#ref: refs/heads/}
  else head=${head[1,7]}; fi
  if [[ -d $g/rebase-merge || -d $g/rebase-apply ]]; then a=rebase
  elif [[ -f $g/MERGE_HEAD ]]; then a=merge
  elif [[ -f $g/CHERRY_PICK_HEAD ]]; then a=cherry-pick
  elif [[ -f $g/BISECT_LOG ]]; then a=bisect
  fi
  _p_branch=" %F{244}${head//\%/%%}%f"
  [[ -n $a ]] && _p_branch+=" %F{167}$a%f"
}
_p_pre() { _p_t0=$SECONDS }
_p_cmd() {
  _p_git
  if (( ${+_p_t0} && SECONDS - _p_t0 > 3 )); then RPROMPT="%F{240}$((SECONDS-_p_t0))s%f"; else RPROMPT=; fi
  unset _p_t0
  print -Pn '\e]0;%~\a'
}
add-zsh-hook preexec _p_pre
add-zsh-hook precmd _p_cmd
setopt prompt_subst
PROMPT='%F{250}󰣇%f %F{255}%~%f${_p_branch} %(?.%F{250}.%F{167})❯%f '

# --- nvm: loaded on first use (was 210 ms per shell) ------------------------
nvm() { unfunction nvm; source $NVM_DIR/nvm.sh; nvm "$@" }

# --- aliases ----------------------------------------------------------------
alias ls='ls --color=auto' grep='grep --color=auto' diff='diff --color=auto'
alias ..='cd ..' ...='cd ../..' .3='cd ../../..' .4='cd ../../../..' .5='cd ../../../../..'
alias mkdir='mkdir -p' sudo='sudo '
alias whatsmyip='curl -4 api.ipify.org && echo'
alias ffsmall='fastfetch --logo-type small'
alias in='sudo pacman -S' un='sudo pacman -Rns' up='sudo pacman -Syu'
alias pl='pacman -Qq' pa='pacman -Ss'
alias fixterm="printf '\e[?1000l\e[?1002l\e[?1003l\e[?1006l\e[?1015l\n'; stty sane; reset"
whence -p code >/dev/null && alias vc='code'
whence -p eza >/dev/null && alias l='eza -lh --icons=auto' ll='eza -lha --icons=auto --sort=name --group-directories-first' ld='eza -lhD --icons=auto' lt='eza --icons=auto --tree'
whence -p bat >/dev/null && alias cat='bat --style=plain --paging=never --color auto'
if whence -p duf >/dev/null; then
  _df() { if [[ $# -ge 1 && -e "${@: -1}" ]]; then duf "${@: -1}"; else duf; fi }
  alias df='_df'
fi

# unknown TERM forwarded over ssh (kitty/st not in this host's terminfo)
_ti=({/usr/share/terminfo,/usr/lib/terminfo,~/.terminfo}/${TERM[1]}/$TERM(N))
if (( ! $#_ti )); then
  case $TERM in *kitty*|st*|*-st*) export TERM=xterm-256color ;; esac
fi
unset _ti

# --- fzf (keys + ffcd/ffe/ffec/ffch) ----------------------------------------
whence -p fzf >/dev/null && source $ZDOT/fzf.zsh

# --- plugins (last: highlighting must wrap every widget) ---------------------
_zp=~/.config/zsh/plugins
ZSH_AUTOSUGGEST_STRATEGY=(history completion)
ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE='fg=240'
ZSH_AUTOSUGGEST_BUFFER_MAX_SIZE=80
ZSH_AUTOSUGGEST_MANUAL_REBIND=1
source $_zp/zsh-autosuggestions/zsh-autosuggestions.zsh
typeset -A ZSH_HIGHLIGHT_STYLES
ZSH_HIGHLIGHT_MAXLENGTH=300
# commands that exist: palette C2 green; not found: red
ZSH_HIGHLIGHT_STYLES=(unknown-token fg=167 command 'fg=#66aa66' builtin 'fg=#66aa66' alias 'fg=#66aa66'
  function 'fg=#66aa66' precommand 'fg=#66aa66,underline' path underline globbing fg=250
  single-quoted-argument fg=246 double-quoted-argument fg=246 comment fg=240
  reserved-word fg=250 arg0 'fg=#66aa66')
source $_zp/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh
unset _zp

# --- startup fetch: spinning Artix logo above the prompt ---------------------
# fetch --pinned (patched build: ~/build/fetch3d, branch "pinned") spins in a
# header while you type. The first command you run freezes it on the face-on
# logo (SIGHUP) and turns it into ordinary text that scrolls up with the
# output. `fetchoff` stops it and gives the rows back. Skipped inside Claude
# Code, nested shells (parent is a shell; SHLVL is already 3 under st), short
# windows.
if [[ -o interactive && -t 0 && -t 1 && -z $CLAUDECODE && $TERM != dumb
      && ${LINES:-0} -ge 20 && $(</proc/$PPID/comm) != (zsh|bash|sh|dash|fish|claude) ]] \
   && whence -p fetch >/dev/null; then
  _fetch=( $(fetch --pinned 2>/dev/null) )    # pid, header rows
  if (( ${#_fetch} == 2 && _fetch[1] > 0 )); then
    zmodload -F zsh/zselect b:zselect
    zmodload -F zsh/system p:sysparams
    _fetch_shell=$sysparams[pid]
    # zshexit also runs in forked copies of this shell (zsh-autosuggestions'
    # completion pty exits after every lookup): only the real shell stops fetch
    _fetch_exit() { [[ $sysparams[pid] == $_fetch_shell ]] && fetchoff }
    # first command: freeze, wait (<= 0.5 s) for the last frame to land so the
    # command's output can't mix into it, then drop the hooks
    _fetch_freeze() {
      local i
      if kill -HUP $_fetch[1] 2>/dev/null; then
        for i in {1..25}; do
          kill -0 $_fetch[1] 2>/dev/null || break
          zselect -t 2
        done
      fi
      fetchoff
    }
    # clear below the header, not the whole screen
    clear() { print -n "\e[$((_fetch[2] + 1));1H\e[J" }
    _fetch_clear_screen() { clear; zle reset-prompt }
    zle -N clear-screen _fetch_clear_screen
    fetchoff() {
      kill $_fetch[1] 2>/dev/null
      print -n '\e7\e[r\e8'
      add-zsh-hook -d preexec _fetch_freeze
      add-zsh-hook -d zshexit _fetch_exit
      zle -A .clear-screen clear-screen 2>/dev/null
      unfunction clear _fetch_clear_screen _fetch_freeze _fetch_exit 2>/dev/null
      unset _fetch _fetch_shell
    }
    add-zsh-hook preexec _fetch_freeze
    add-zsh-hook zshexit _fetch_exit
  else
    unset _fetch
  fi
fi
# c: take the fetch header away for this shell, then clear everything
c() {
  (( ${+functions[fetchoff]} )) && fetchoff
  command clear && printf '\e[3J'
}

# --- keep sourced files byte-compiled -----------------------------------------
# `source` (and zsh itself for ~/.zshrc) uses file.zwc when it is newer than
# file; recompile in the background whenever a file changes.
() {
  local f
  for f in ~/.zshrc $ZDOT/fzf.zsh $ZCACHE/fzf-init.zsh \
           $ZDOT/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh \
           $ZDOT/plugins/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh \
           $ZDOT/plugins/zsh-syntax-highlighting/highlighters/*/*-highlighter.zsh(N); do
    [[ -r $f && ! $f.zwc -nt $f ]] || continue
    zcompile -R $f.$$.zwc $f 2>/dev/null && mv -f $f.$$.zwc $f.zwc
  done
} &!
