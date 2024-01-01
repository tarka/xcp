_xcp() {
  local cur prev words cword
  _init_completion || return

  # do not suggest options after --
  local i
  for ((i = 1; i < cword; i++)); do
    if [[ ${words[$i]} == -- ]]; then
      _filedir
      return
    fi
  done

  local options=(
    -T
    -g
    -h
    -n
    -r
    -v
    -w
    "$(_parse_help "$1" -h)" # long options will be parsed from `--help`
  )
  local units='B K M G' # in line with most completions prefer M to MB/MiB
  local drivers='parfile parblock'
  local reflink='auto always never'

  case "$prev" in
  -h | --help) return ;;

  --block-size)
    if [[ -z $cur ]]; then
      COMPREPLY=(1M) # replace "nothing" with the default block size
    else
      local num="${cur%%[^0-9]*}" # suggest unit suffixes after numbers
      local unit="${cur##*[0-9]}"
      COMPREPLY=($(compgen -P "$num" -W "$units" -- "$unit"))
    fi
    return
    ;;

  --reflink)
    COMPREPLY=($(compgen -W "$reflink" -- "$cur"))
    return
    ;;

  --driver)
    COMPREPLY=($(compgen -W "$drivers" -- "$cur"))
    return
    ;;

  -w | --workers)
    COMPREPLY=($(compgen -W "{0..$(_ncpus)}" -- "$cur")) # 0 == auto
    return
    ;;
  esac

  if [[ $cur == -* ]]; then
    COMPREPLY=($(compgen -W "${options[*]}" -- "$cur"))
    return
  fi

  _filedir # suggest files if nothing else matched
} && complete -F _xcp xcp

# vim: sw=2 sts=2 et ai ft=bash
# path: /usr/share/bash-completion/completions/xcp
