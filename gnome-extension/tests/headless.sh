#!/bin/bash
# Live test of the extension in a headless GNOME Shell with its own session
# bus and temporary XDG directories; the user's session is not touched.
#
#   gnome-extension/tests/headless.sh PATH/TO/wsdbus-ctl
#
# With WSDBUS_HEADLESS_SANDBOX=1 the test also waits up to 60 s for a
# sandboxed process to call the provider: it writes the bus address to
# $work/address and expects the caller's exit status in $work/sandboxed
# (7, Denied). The work directory lives under ~/.cache/ws-dev so a Flatpak
# app with access to the home directory can reach the bus socket.
set -u
ctl=$(realpath "${1:?usage: headless.sh PATH/TO/wsdbus-ctl}")
here=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$HOME/.cache/ws-dev"
work=$(mktemp -d -p "$HOME/.cache/ws-dev" wsdbus-headless.XXXXXX)
cleanup() {
    # Whatever the session left behind: processes started with the work
    # directory in their arguments, then mounts under it.
    local pids
    pids=$(ps -eo pid=,args= | grep -F "$work" | grep -v grep | awk '{print $1}')
    [ -n "$pids" ] && kill $pids 2>/dev/null && sleep 1
    grep -F "$work" /proc/self/mountinfo | cut -d' ' -f5 | while read -r mount; do
        fusermount3 -uz "$mount" 2>/dev/null
    done
    rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
uuid=wsdbus@kkkingqz.github.io
mkdir -p "$work"/{config,data,cache,state,bus} "$work/run" "$work/data/gnome-shell/extensions"
chmod 700 "$work/run"
cp -r "$here" "$work/data/gnome-shell/extensions/$uuid"
rm -rf "$work/data/gnome-shell/extensions/$uuid/tests"
# The session configuration with one socket in the work directory; only
# dconf and the Shell screen saver are activatable, so no gvfs, portals or
# other session services start.
mkdir -p "$work/services"
cp /usr/share/dbus-1/services/ca.desrt.dconf.service \
    /usr/share/dbus-1/services/org.gnome.ScreenSaver.service "$work/services/"
sed -e "s|<listen>.*</listen>|<listen>unix:dir=$work/bus</listen>|" \
    -e "s|<standard_session_servicedirs />|<servicedir>$work/services</servicedir>|" \
    /usr/share/dbus-1/session.conf >"$work/bus.conf"

export XDG_CONFIG_HOME=$work/config XDG_DATA_HOME=$work/data XDG_CACHE_HOME=$work/cache \
    XDG_STATE_HOME=$work/state XDG_RUNTIME_DIR=$work/run CTL=$ctl WORK=$work
unset WAYLAND_DISPLAY DISPLAY
dbus-run-session --config-file="$work/bus.conf" -- bash -c '
set -u
log=$XDG_STATE_HOME/shell.log
out=$XDG_STATE_HOME/out
gsettings set org.gnome.shell disable-user-extensions false
gsettings set org.gnome.shell enabled-extensions "[\"wsdbus@kkkingqz.github.io\"]"
gsettings set org.gnome.desktop.interface color-scheme prefer-dark
gsettings set org.gnome.desktop.interface accent-color teal
gnome-shell --headless --wayland --no-x11 --virtual-monitor 1280x800 >"$log" 2>&1 &
shell=$!
for _ in $(seq 100); do "$CTL" info >/dev/null 2>&1 && break; sleep 0.1; done
failed=0
check() { # NAME EXPECTED-STATUS COMMAND...
    local name=$1 expected=$2; shift 2
    "$@" >"$out" 2>&1; local status=$?
    if [ "$status" = "$expected" ]; then echo "PASS $name"
    else echo "FAIL $name: status $status, expected $expected"; cat "$out"; failed=1; fi
}
has() { # NAME PATTERN: the last output contains PATTERN
    if grep -q -- "$2" "$out"; then echo "PASS $1"
    else echo "FAIL $1: no $2 in"; cat "$out"; failed=1; fi
}
lacks() {
    if grep -q -- "$2" "$out"; then echo "FAIL $1: $2 in"; cat "$out"; failed=1
    else echo "PASS $1"; fi
}
wait_for() { # NAME PATTERN COMMAND...: poll until the output contains PATTERN
    local name=$1 pattern=$2; shift 2
    for _ in $(seq 50); do "$@" >"$out" 2>&1; grep -q -- "$pattern" "$out" && break; sleep 0.2; done
    has "$name" "$pattern"
}

check info 0 "$CTL" info; has provider "provider gnome"
check state 0 "$CTL" --json state
has scheme "\"appearance.scheme\":\"dark\""; has accent "\"appearance.accent\":\"#2190a4\""
has locked "\"session.locked\":false"; has workspaces "\"workspaces\":\[{"
has layout "\"keyboard.layout\":"
lacks "no brightness without a backlight" "display.brightness"
check "brightness unsupported" 4 "$CTL" brightness 0.5
check "unknown workspace" 2 "$CTL" workspace 99
check "unknown app" 2 "$CTL" launch no.such.app
check "unknown key" 2 "$CTL" key NoSuchKey press
check "stale window" 5 "$CTL" window 999999 close
check "mode unsupported" 4 "$CTL" mode 1 floating
check "key press" 0 "$CTL" key Escape press
check "repeated press" 0 "$CTL" key Escape press
check "key release" 0 "$CTL" key Escape release
check "media key" 0 "$CTL" key XF86AudioPlay release
check shortcut 0 "$CTL" shortcut Control_L t
check "shortcut for another app" 0 "$CTL" shortcut --app no.such.app Control_L t

gsettings set org.gnome.desktop.interface color-scheme default
sleep 0.5
check "scheme follows settings" 0 "$CTL" --json state; has light "\"appearance.scheme\":\"light\""
"$CTL" --json watch >"$XDG_STATE_HOME/watch" 2>&1 &
watch=$!
sleep 0.5
gsettings set org.gnome.desktop.interface color-scheme prefer-dark
sleep 0.5

check launch 0 "$CTL" launch org.gnome.Calculator
wait_for "window appears" "\"app\":\"org.gnome.Calculator\"" "$CTL" --json windows
id=$(grep -o "\"id\":\"[0-9]*\"" "$out" | head -1 | grep -o "[0-9]*")
wait_for "focus follows" "\"focus.app\":\"org.gnome.Calculator\"" "$CTL" --json state
check minimize 0 "$CTL" window "$id" minimize
wait_for "minimized" "\"minimized\":true" "$CTL" --json windows
check restore 0 "$CTL" window "$id" restore
wait_for "restored" "\"minimized\":false" "$CTL" --json windows
check activate 0 "$CTL" window "$id" activate
check "toggle maximize" 0 "$CTL" window "$id" toggle-maximize
wait_for "maximized" "\"maximized\":true" "$CTL" --json windows
check workspace 0 "$CTL" workspace 1
check "focused app shortcut" 0 "$CTL" shortcut --app org.gnome.Calculator Escape
check close 0 "$CTL" window "$id" close
wait_for "window gone" "\"windows\":\[\]" "$CTL" --json windows
check "closed window is stale" 5 "$CTL" window "$id" activate
kill $watch
if grep -q "\"facts\":{\"appearance.scheme\":\"dark\"}" "$XDG_STATE_HOME/watch"; then echo "PASS watch facts"
else echo "FAIL watch facts"; cat "$XDG_STATE_HOME/watch"; failed=1; fi
if grep -q "\"removed\":\[\"$id\"\]" "$XDG_STATE_HOME/watch"; then echo "PASS watch windows"
else echo "FAIL watch windows"; cat "$XDG_STATE_HOME/watch"; failed=1; fi

if [ "${WSDBUS_HEADLESS_SANDBOX:-}" = 1 ]; then
    echo "$DBUS_SESSION_BUS_ADDRESS" >"$WORK/address"
    for _ in $(seq 300); do [ -s "$WORK/sandboxed" ] && break; sleep 0.2; done
    if [ "$(cat "$WORK/sandboxed" 2>/dev/null)" = 7 ]; then echo "PASS sandboxed caller denied"
    else echo "FAIL sandboxed caller: status $(cat "$WORK/sandboxed" 2>/dev/null)"; failed=1; fi
fi

check "key press before lock" 0 "$CTL" key Shift_L press
gdbus call --session --dest org.gnome.ScreenSaver --object-path /org/gnome/ScreenSaver \
    --method org.gnome.ScreenSaver.Lock >"$out" 2>&1 || { echo "lock call:"; cat "$out"; }
wait_for "locked" "\"session.locked\":true" "$CTL" --json state
check "launch while locked" 6 "$CTL" launch org.gnome.Calculator
check "press while locked" 6 "$CTL" key Escape press
check "release while locked" 0 "$CTL" key Shift_L release
check "state while locked" 0 "$CTL" state

kill $shell; wait $shell 2>/dev/null
if grep -i "wsdbus" "$log" | grep -iE "error|warn|exception|fail"; then echo "FAIL shell log"; failed=1; fi
if [ $failed = 0 ]; then echo "headless: all passed"
else echo "--- shell log"; grep -vE "^dbus-daemon" "$log" | tail -40; fi
exit $failed
' 2>/dev/null
