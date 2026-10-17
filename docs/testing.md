# Проверки

## Автоматические

| Что | Команда | Где |
|---|---|---|
| Rust: контракт, клиент, `wsdbus-ctl` против поддельного поставщика на частной шине | `cargo test --workspace --locked` | CI, локально |
| GJS: модули расширения без GNOME Shell, случаи `spec/` | `gjs -m gnome-extension/tests/run-tests.js` | CI, локально |
| Расширение в настоящем GNOME Shell 50 без экрана, своя шина и временные каталоги | `gnome-extension/tests/headless.sh target/debug/wsdbus-ctl` | локально, нужен GNOME Shell 50 |

`headless.sh` с `WSDBUS_HEADLESS_SANDBOX=1` ждёт вызова от процесса в
песочнице: пишет адрес шины в `$work/address` и ждёт код выхода в
`$work/sandboxed` (ожидается 7). Так проверено, что `wsdbus-ctl` изнутри
Flatpak читает факты, а `launch`, `key`, `shortcut` получают `Denied`.

## Живая сессия GNOME (mbp16)

Только с подтверждения владельца: расширение ставится в сессию.

1. Установить расширение в `~/.local/share/gnome-shell/extensions/wsdbus@kkkingqz.github.io`,
   перелогиниться, `gnome-extensions enable wsdbus@kkkingqz.github.io`.
2. `wsdbus-ctl info` — `provider gnome`.
3. `wsdbus-ctl watch` в терминале; переключить окна, столы, тему, акцент,
   раскладку, яркость клавишами — каждое изменение видно одной строкой.
4. `wsdbus-ctl windows`; для окна: `minimize`, `restore`, `toggle-maximize`,
   `activate`, `close`.
5. `wsdbus-ctl brightness 0.5` — яркость меняется, факт следует.
6. `wsdbus-ctl key XF86AudioPlay press` и `release` — плеер реагирует.
   Сочетание уходит в окно в фокусе, а это терминал с командой, поэтому
   нужна задержка: `sleep 3; wsdbus-ctl shortcut --app org.gnome.Ptyxis Control_L Shift_L t`
   и за 3 секунды переключиться в Ptyxis — откроется вкладка. То же с
   переключением в другое окно — ничего не происходит.
7. Заблокировать экран: `watch` показывает `session.locked true`,
   `wsdbus-ctl launch org.gnome.Calculator` — код 6; после разблокировки
   нажатые клавиши не залипли.
8. Из приложения Flatpak с доступом к сессионной шине `wsdbus-ctl launch …`
   — код 7, `state` работает.
9. Отключить и включить расширение: `watch` показывает `lost`, затем новый
   снимок.
10. Под русской раскладкой повторить шаг 6 (открыто: до `ffdfbb4` вкладка не
    открывалась; после повторного входа проверить снова).

## Этап 3: потребители (mbp16)

После `ws apply` с ядром wsconfig `69d021b` и выше (с подтверждения
владельца). Расширение wsdbus теперь ставит Home Manager ссылками в store;
до повторного входа работает прежняя копия, она того же кода.

1. `wsdbus-ctl info` (из профиля) и `~/.local/share/wsconfig/libexec/ws-window check`
   — `wsdbus provider: windows, WindowAction`.
2. Сочетания `ws-window` (раскладка macOS): Command+H — окна приложения в
   фокусе свёрнуты; Option+Command+H — свёрнуты остальные; Option+Command+W
   — окна приложения закрыты, окно в фокусе последним; Option+Command+M —
   как Command+H.
3. `systemctl --user show workstation-widgets -p Environment` содержит
   `WIDGETS_DESKTOP_CTL`; открыть виджет на панели и заблокировать экран —
   после разблокировки popup закрыт.
4. После проверки 2: убрать `window-control@carlo9890.github.io` из
   `~/wsconfig/gnome/config.toml`, `ws apply`, повторить 2. Файлы прежних
   расширений убрать: `gnome-extensions uninstall window-control@carlo9890.github.io`,
   `gnome-extensions uninstall window-monitor-pro@muhammed.hussien2030.gmail.com`.
5. `ws check` — зелёный.
