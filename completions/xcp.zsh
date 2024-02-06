#compdef xcp

typeset -A opt_args

_xcp() {
  local -a args

  # short + long
  args+=(
    '(- *)'{-h,--help}'[Print help]'
    '*'{-v,--verbose}'[Increase verbosity (can be repeated)]'
    {-T,--no-target-directory}'[Overwrite target directory, do not create a subdirectory]'
    {-g,--glob}'[Expand (glob) filename patterns]'
    {-n,--no-clobber}'[Do not overwrite an existing file]'
    {-r,--recursive}'[Copy directories recursively]'
    {-w,--workers}'[Workers for recursive copies (0=auto)]:workers:_values workers {0..$(getconf _NPROCESSORS_ONLN)}'
  )

  # long
  args+=(
    --block-size'[Block size for file operations]: :_numbers -u bytes -d 1M size B K M G'
    --driver'[How to parallelise file operations]:driver:((
      parfile\:"parallelise at the file level (default)"
      parblock\:"parallelise at the block level"
    ))'
    --reflink'[Whether and how to use reflinks]:reflink:((
      auto\:"attempt to reflink and fallback to a copy (default)"
      always\:"return an error if it cannot reflink"
      never\:"always perform a full data copy"
    ))'
    --backup'[Whether to create backups of overwritten files]:backup:((
      none\:"no backups (default)"
      numbered\:"follow the semantics of cp numbered backups"
      auto\:"create a numbered backup if previous backup exists"
    ))'
    --fsync'[Sync each file to disk after it is written]'
    --gitignore'[Use .gitignore if present]'
    --no-perms'[Do not copy file permissions]'
    --no-timestamps'[Do not copy file timestamps]'
    --no-progress'[Disable progress bar]'
  )

  # positional
  args+=(
    '*:paths:_files'
  )

  _arguments -s -S $args
}

_xcp "$@"

# vim: sw=2 sts=2 et ai ft=zsh
# path: /usr/share/zsh/site-functions/_xcp
