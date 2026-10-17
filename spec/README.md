# Контракт `Desktop1`

Нормативное описание контракта wsdbus v1. Интерфейс —
[`io.github.kkkingqz.WsDbus.Desktop1.xml`](io.github.kkkingqz.WsDbus.Desktop1.xml),
общие тестовые случаи — [`appid-cases.json`](appid-cases.json) и
[`sandbox-cases.json`](sandbox-cases.json). Их проходят `wsdbus-contract` и
каждый поставщик. Решения — в [журнале](../docs/decisions.md).

## Имена

| Что | Значение |
|---|---|
| имя на сессионной шине | `io.github.kkkingqz.WsDbus` |
| объект | `/io/github/kkkingqz/WsDbus` |
| интерфейс | `io.github.kkkingqz.WsDbus.Desktop1` |

В сессии работает один поставщик. Несовместимое изменение — новый интерфейс
`Desktop2` рядом со старым. Совместимые дополнения (новые факты, ключи окон,
возможности) входят в `Desktop1`: потребитель пропускает незнакомые имена.

## Свойства

| Свойство | Тип | Смысл |
|---|---|---|
| `Version` | `u` | `1` |
| `Provider` | `s` | `gnome`, `hyprland`; поддельный поставщик — `fake` |
| `Capabilities` | `as` | поддерживаемые факты (по имени), `windows` и методы-команды (по имени метода) |

Неподдерживаемого факта просто нет. Неподдерживаемая команда отвечает
`Unsupported`. Без возможности `windows` потребитель не вызывает
`GetWindows`.

## Факты

`GetState() → (t revision, a{sv} facts)` — все присутствующие факты.
`StateChanged(t revision, a{sv} changed, as removed)` — изменившиеся и
исчезнувшие факты.

| Факт | Тип D-Bus | Правило значения |
|---|---|---|
| `focus.app` | `s` | `app.id` окна в фокусе; без фокуса факта нет |
| `focus.class` | `s` | исходный class / WM class окна в фокусе |
| `focus.fullscreen` | `b` | окно в фокусе на весь экран |
| `session.locked` | `b` | экран заблокирован |
| `appearance.scheme` | `s` | `light` или `dark` |
| `appearance.accent` | `s` | `#rrggbb`, строчные цифры |
| `appearance.font` | `s` | шрифт интерфейса, как в настройках окружения |
| `workspaces` | `aa{sv}` | записи столов, см. ниже |
| `workspace.active` | `s` | id активного стола монитора в фокусе |
| `display.brightness` | `d` | от 0.0 до 1.0 |
| `keyboard.layout` | `s` | активная раскладка (`us`, `ru`, `ua` или id источника ввода), только чтение |

Текстовые факты непустые. Запись стола: `id` (`s`, непустой, уникальный),
`name` (`s`), `monitor` (`s`), `active` (`b`, активен на своём мониторе),
`mode` (`s`, `tiling` или `floating`; только у поставщика, который поддерживает
`SetWorkspaceMode`).

Факт известного имени с неверным типом или значением потребитель отбрасывает
и сообщает о нём; остальные факты сообщения применяются.

## Окна

`GetWindows() → (t revision, aa{sv} windows)`,
`WindowsChanged(t revision, aa{sv} upserted, as removed)`.

| Ключ | Тип | Смысл |
|---|---|---|
| `id` | `s` | обязательный, непрозрачный, уникален в пределах сессии |
| `app` | `s` | обязательный, `app.id` |
| `class`, `title`, `workspace`, `monitor` | `s` | по умолчанию пустая строка |
| `focused`, `minimized`, `maximized`, `floating`, `fullscreen` | `b` | по умолчанию `false` |

`upserted` содержит окна целиком. Id не переиспользуется для другого окна,
пока сессия жива: в GNOME он из `Meta.Window.get_stable_sequence()`, в
Hyprland — из `stableId` и подписи экземпляра.

## Ревизия

Одна ревизия на всё состояние. Каждое изменение увеличивает её ровно на 1 и
порождает ровно один сигнал (`StateChanged` или `WindowsChanged`). Изменение
без разницы не меняет ревизию и сигнала не даёт. После перезапуска поставщик
начинает счёт заново; потребитель узнаёт об этом по смене владельца имени.

Потребитель:

1. подписывается на `NameOwnerChanged` имени и на оба сигнала;
2. берёт `GetState` (ревизия `rf`), затем `GetWindows` (ревизия `rw`);
3. применяет сигнал фактов с ревизией больше `rf`, сигнал окон — больше `rw`,
   остальные пропускает как уже вошедшие в снимок;
4. ревизия больше последней известной плюс 1 — пропуск: снимок заново;
5. принимает сигналы только от текущего уникального владельца имени;
6. при исчезновении владельца забывает всё состояние, при новом владельце
   берёт снимок.

Это реализуют `Mirror` (`wsdbus-contract`) и `Watcher` (`wsdbus-client`).

## Команды

| Метод | Смысл |
|---|---|
| `Shortcut(as keys, s app)` | нажать `keys` по порядку и отпустить в обратном. Непустой `app` — только если это приложение в фокусе, иначе ничего не происходит. От 1 до 8 клавиш; имена — XKB keysym (`Control_L`, `Shift_L`, `Alt_L`, `Super_L`, `t`, `F5`, `Return`) |
| `LaunchApp(s id)` | запустить приложение по `app.id` |
| `ActivateWorkspace(s id)` | перейти на стол |
| `WindowAction(s id, s action)` | `activate`, `close`, `minimize`, `restore`, `toggle-maximize` |
| `SetWorkspaceMode(s id, s mode)` | `tiling` или `floating`; только Hyprland с `policy` |
| `Key(s key, b pressed)` | нажать (`true`) или отпустить (`false`) одну клавишу; имя — XKB keysym (`Escape`, `F5`, `XF86AudioPlay`, `XF86MonBrightnessUp`). Для поверхностей без своего ввода клавиш (wstouchbar). Клавиши, которые вызывающий не отпустил, поставщик отпускает сам, когда вызывающий уходит с шины или экран блокируется |
| `SetBrightness(d level)` | яркость экрана от 0.0 до 1.0; та же возможность, что и факт `display.brightness` |

Перемещения, размера и геометрии окон в v1 нет.

Проверки идут в таком порядке; первая неуспешная даёт ошибку:

| Порядок | Проверка | Ошибка |
|---|---|---|
| 1 | аргументы: пустой id, пустой `app` при его наличии, число и вид клавиш, неизвестное действие или режим, уровень яркости вне 0.0–1.0 | `org.freedesktop.DBus.Error.InvalidArgs` |
| 2 | вызывающий не в песочнице | `io.github.kkkingqz.WsDbus.Error.Denied` |
| 3 | метод в `Capabilities`, действие поддерживается | `io.github.kkkingqz.WsDbus.Error.Unsupported` |
| 4 | экран не заблокирован — только `Shortcut`, `LaunchApp` и нажатие в `Key` (отпускание разрешено всегда) | `io.github.kkkingqz.WsDbus.Error.Locked` |
| 5 | окно с этим id есть | `io.github.kkkingqz.WsDbus.Error.StaleWindow` |
| 5 | стол, приложение или клавиша с этим именем есть | `org.freedesktop.DBus.Error.InvalidArgs` |

Проверки 1–4 — функция `admit` в `wsdbus-contract`; поставщик на другом языке
повторяет их в том же порядке.

## `app.id`

Id desktop-файла без `.desktop`. Нет desktop-файла — `wmclass:<class>`, class
без изменений регистра. Id GNOME вида `window:N` означает отсутствие
desktop-файла. Пробелы по краям отбрасываются. Случаи —
[`appid-cases.json`](appid-cases.json): поля `desktop_id` и `class` (`null` —
нет значения), ожидаемое `app`.

## Доверие

Чтение свойств, фактов и окон открыто всем. Изменяющие методы поставщик
проверяет по отправителю:

1. PID через `org.freedesktop.DBus.GetConnectionUnixProcessID`; не получен —
   не проверен;
2. `/proc/PID/root/.flatpak-info` есть — Flatpak; ошибка, кроме «нет файла», —
   не проверен;
3. `/proc/PID/cgroup` не прочитан — не проверен; в пути есть юнит
   `app-flatpak-*` — Flatpak, `snap.*` — Snap;
4. иначе процесс доверенный.

Процессу в песочнице и непроверенному — `Denied`. Случаи —
[`sandbox-cases.json`](sandbox-cases.json): `pid`, `flatpak_info`
(`present`, `absent`, `failed`), `cgroup` (`probe` и `text`), ожидаемый
`verdict` (`trusted`, `sandboxed`, `unverified`).

## JSON `wsdbus-ctl`

`wsdbus-ctl --json` пишет по объекту на строку. Факты — объект по именам фактов
(текст — строка, логический — `true`/`false`, число — число, `workspaces` —
массив объектов записи), окно — объект с ключами окна.

| Строка | Когда |
|---|---|
| `{"snapshot":true,"revision":N,"provider":…,"version":1,"capabilities":[…],"facts":{…},"windows":[…]}` | `watch`: появился поставщик или пропуск ревизии; заменяет всё прежнее |
| `{"revision":N,"facts":{…},"removed":[…]}` | `watch`: изменение фактов |
| `{"revision":N,"windows":[…],"removed":[…]}` | `watch`: изменение окон |
| `{"lost":true}` | `watch`: поставщик пропал, всё прежнее недействительно |
| `{"revision":N,"facts":{…}}` | `state` |
| `{"revision":N,"windows":[…]}` | `windows` |
| `{"provider":…,"version":1,"capabilities":[…]}` | `info` |

Коды выхода: 0 — успех, 1 — ошибка шины или протокола, 2 — неверный вызов или
`InvalidArgs`, 3 — нет поставщика, 4 — `Unsupported`, 5 — `StaleWindow`,
6 — `Locked`, 7 — `Denied`.

Команды `wsdbus-ctl` для новых методов: `key KEY press|release`,
`brightness LEVEL`.
