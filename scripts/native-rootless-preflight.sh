#!/bin/sh
# Diagnostic-only, bounded inner-container facts; never print names or raw host files.
set -u

proc_root=${NATIVE_PREFLIGHT_PROC_ROOT:-/proc/self}
etc_root=${NATIVE_PREFLIGHT_ETC_ROOT:-/etc}
printf 'native-rootless-preflight begin\n'
printf 'identity uid=%s gid=%s\n' "$(id -u)" "$(id -g)"

print_map() {
  label=$1
  file=$2
  if [ ! -r "$file" ]; then
    printf '%s unavailable\n' "$label"
    return
  fi
  awk -v label="$label" '
    NR <= 4 && NF == 3 && $1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]+$/ && $3 ~ /^[0-9]+$/ &&
      length($1) <= 20 && length($2) <= 20 && length($3) <= 20 {
      outside = $2 < 65536 ? "low-id-redacted" : $2
      printf "%s inside=%s outside=%s length=%s\n", label, $1, outside, $3
      found = 1
    }
    END { if (!found) printf "%s unavailable\n", label }
  ' "$file" || printf '%s unavailable\n' "$label"
}
print_map uid_map "$proc_root/uid_map"
print_map gid_map "$proc_root/gid_map"

if [ -r "$proc_root/status" ]; then
  awk '
    NR <= 128 && ($1 == "NoNewPrivs:" || $1 == "Seccomp:") && !seen[$1]++ {
      if ($2 ~ /^[0-9]+$/ && length($2) <= 3) printf "%s %s\n", $1, $2
    }
    NR <= 128 && ($1 == "CapEff:" || $1 == "CapBnd:") && !seen[$1]++ {
      if ($2 ~ /^[0-9A-Fa-f]+$/ && length($2) <= 16) printf "%s %s\n", $1, $2
    }
  ' "$proc_root/status" || printf 'status unavailable\n'
else
  printf 'status unavailable\n'
fi

getcap_command=${NATIVE_PREFLIGHT_GETCAP:-$(command -v getcap 2> /dev/null || true)}
print_helper() {
  label=$1
  file=$2
  expected=$3
  if [ -z "$file" ] || [ ! -f "$file" ]; then
    printf '%s unavailable\n' "$label"
    return
  fi
  ownership=$(stat -c '%u:%g:%a' -- "$file" 2> /dev/null || true)
  case "$ownership" in
    *[!0-9:]* | '') printf '%s unavailable\n' "$label" ;;
    *) printf '%s owner-group-mode=%s\n' "$label" "$ownership" ;;
  esac
  if [ -z "$getcap_command" ]; then
    printf '%s file-capability=unavailable\n' "$label"
    return
  fi
  # Report only a closed classification, never helper paths or raw xattrs.
  if capabilities=$(timeout 2s "$getcap_command" -n -- "$file" 2> /dev/null); then
    case "$capabilities" in
      '') classification=none ;;
      "$file $expected") classification=expected ;;
      *) classification=other-redacted ;;
    esac
    [ "${#capabilities}" -le 256 ] || classification=other-redacted
  else
    classification=unavailable
  fi
  printf '%s file-capability=%s\n' "$label" "$classification"
}
newuidmap=${NATIVE_PREFLIGHT_NEWUIDMAP:-$(command -v newuidmap 2> /dev/null || true)}
newgidmap=${NATIVE_PREFLIGHT_NEWGIDMAP:-$(command -v newgidmap 2> /dev/null || true)}
print_helper newuidmap "$newuidmap" cap_setuid=ep
print_helper newgidmap "$newgidmap" cap_setgid=ep

owner=$(id -un 2> /dev/null || true)
uid=$(id -u)
print_ranges() {
  label=$1
  file=$2
  if [ ! -r "$file" ]; then
    printf '%s unavailable\n' "$label"
    return
  fi
  awk -F: -v label="$label" -v owner="$owner" -v uid="$uid" '
    ($1 == owner || $1 == uid) && $2 ~ /^[0-9]+$/ && $3 ~ /^[0-9]+$/ &&
      length($2) <= 20 && length($3) <= 20 {
      if (found < 4) printf "%s start=%s length=%s\n", label, $2, $3
      found++
    }
    END {
      if (!found) printf "%s none\n", label
      if (found > 4) printf "%s additional-ranges-redacted\n", label
    }
  ' "$file" || printf '%s unavailable\n' "$label"
}
print_ranges subuid "$etc_root/subuid"
print_ranges subgid "$etc_root/subgid"
printf 'native-rootless-preflight end\n'
