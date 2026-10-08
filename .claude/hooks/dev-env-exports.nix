# Turns the JSON from `nix print-dev-env --json` into bash `export` lines.
#
# Only exported variables are kept, minus those `nix develop` also ignores
# because they describe the build sandbox rather than the dev shell (for
# example SSL_CERT_FILE, which points to a dummy file there). The dev shell's
# PATH is put in front of the existing PATH instead of replacing it.
jsonFile:
let
  env = builtins.fromJSON (builtins.readFile jsonFile);
  ignored = [
    "BASHOPTS" "HOME" "NIX_BUILD_TOP" "NIX_ENFORCE_PURITY" "NIX_LOG_FD" "NIX_REMOTE" "PPID" "SHELL"
    "SHELLOPTS" "SSL_CERT_FILE" "TEMP" "TEMPDIR" "TERM" "TMP" "TMPDIR" "TZ" "UID"
  ];
  vars = builtins.removeAttrs env.variables ignored;
  names = builtins.filter (n: vars.${n}.type == "exported") (builtins.attrNames vars);
  quote = s: "'" + builtins.replaceStrings [ "'" ] [ "'\\''" ] s + "'";
  line = n:
    if n == "PATH"
    then "export PATH=${quote vars.PATH.value}:\"$PATH\""
    else "export ${n}=${quote vars.${n}.value}";
in
builtins.concatStringsSep "\n" (map line names) + "\n"
