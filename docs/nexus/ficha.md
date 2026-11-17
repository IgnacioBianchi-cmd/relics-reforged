# Ficha para Nexus Mods

Todo lo que pide Nexus al crear la página y subir el archivo. La descripción larga está en
[`descripcion.bbcode`](descripcion.bbcode) (Nexus usa BBCode: se pega tal cual en el editor, en
modo BBCode).

## Página

| Campo | Valor |
|---|---|
| Juego | Elden Ring Nightreign |
| Nombre | Relics Reforged |
| Versión | 1.0.0 |
| Autor | Ignacio |
| Categoría | QoL. Las de Nightreign son Armour, Elden Right Nightreign, Gameplay, Miscellaneous, QoL y Skins. Gameplay agrupa mods que cambian las partidas (combate, escalado, movesets) y Miscellaneous es el cajón de sastre; este no toca las partidas, recorta el farmeo de reliquias con un costo, como Free Reign (volver a pelear jefes desde la Roundtable), que también está en QoL |
| Idioma | English (con sección en español) |
| Contenido para adultos | No |

**Resumen** (Nexus lo limita a unos 350 caracteres; este tiene 265):

> Transfer one effect from a relic you don't need onto one you want to keep, inside Relic Rites. The result is always a relic the game could have rolled. Costs murk and 3 sacrifices. Mouse, keyboard and controller. Works offline and with Seamless Co-op. Requires me3; ME3 Manager ready.

**Requisitos** (sección "Requirements" de Nexus):

- me3 (https://me3.help; no tiene página en Nexus).
- Opcional: Mod Engine 3 Manager (https://www.nexusmods.com/eldenringnightreign/mods/213).
- Opcional: Seamless Co-op para Nightreign (enlace a su página en Nexus).

**Permisos** (decisión tuya; lo habitual para un mod con código abierto):

- Subir el mod a otros sitios: pedir permiso.
- Modificar o reutilizar partes: permitido con crédito (el código es MIT).
- Convertir a otros juegos: no aplica.
- Uso en mods pagos o con donaciones: a elección.

## Archivos

Dos archivos, como los demás mods DLL: el principal es la DLL sola (para agregar al perfil que
ya use cada uno) y el opcional trae un perfil de me3 para quien no tiene otros mods. El principal
no lleva `.me3`: ME3 Manager importa como perfil cualquier zip que traiga uno, en vez de
instalarlo como mod DLL. Los dos salen de `powershell -File tools/package.ps1`.

| Campo | Principal | Opcional |
|---|---|---|
| Archivo | `target/dist/RelicsReforged-v1.0.0.zip` | `target/dist/RelicsReforged-Standalone-v1.0.0.zip` |
| Contenido | `RelicsReforged.dll` + `RelicsReforged/` (README, licencia) en la raíz | Lo mismo + `RelicsReforged.me3`, dentro de `RelicsReforged-Standalone/` |
| Nombre del archivo | Relics Reforged | Relics Reforged - Standalone (me3 profile) |
| Versión | 1.0.0 | 1.0.0 |
| Categoría | Main Files | Optional Files |
| Descripción | DLL mod. Install it with ME3 Manager, or add RelicsReforged.dll to your own me3 profile (with Seamless Co-op too). Requires me3 0.13+ and game version 1.3.3. | For players without other mods: unzip anywhere and double-click RelicsReforged.me3. Plays offline on a copy of your save. Requires me3 0.13+. |

**Changelog 1.0.0**

- First release: effect transfer between relics in Relic Rites, with preview, double confirmation, forge limits and save backups.
- In-game menu in the style of the game, with search, filter, sorting, icons and sounds from the game.
- Mouse, keyboard and controller (Steam Input) support; the game doesn't receive inputs while the forge is open.
- English and Spanish.
- Settings file (relics_reforged.ini): open key and button, sacrifices, points, murk, forge limit, language and save backups.
- Regular DLL mod layout (ME3 Manager, your own me3 profile, Seamless Co-op) and an optional standalone me3 profile.

## Imágenes

Nexus usa la primera como portada; formato 16:9, sin el contador de FPS de arriba a la izquierda.

1. **Portada:** la forja con la vista previa completa (receptora, donante, 3 sacrificios y el resultado con el efecto en dorado).
2. Ritos de reliquias con "F7: Forge relics" en la barra de abajo (cómo se abre).
3. La lista de reliquias con íconos y marcas ("[forged 1]", "[in use: favorite]").
4. El filtro con una condición de efecto abierta.
5. Antes y después: la reliquia en Relic Rites antes y después de forjar.
6. Opcional: el menú con los botones del mando.

Conviene sacarlas con el juego en inglés (la mayoría del público de Nexus) y repetir alguna en
español para la sección en español.

## Antes de publicar

- [x] Link al código en la descripción: https://github.com/IgnacioBianchi-cmd/relics-reforged.
- [x] Instalar el zip principal en ME3 Manager (arrastrándolo) con Seamless como mod externo con `load_early`, y forjar (ME3 Manager 1.5.2, Seamless 1.1.3, 2026-11-17).
- [x] Probar el zip principal en `nightreign-default.me3` (sin ME3 Manager) y el standalone (2026-11-17).
- [x] Sesión de Seamless con un amigo, los dos con el mod (2026-11-17): sin errores, incluso con una versión distinta del mod en cada PC.
- [x] Sesión de Seamless con un jugador sin el mod (2026-11-17): el que lo tenía forjó sin problemas.
- [x] Probar en otra PC: la del amigo, en la misma sesión de Seamless (2026-11-17).
- [x] Etiquetar la versión (`v1.0.0`) en `main` (subido a GitHub el 2026-11-17).
- [ ] Sacar las capturas.
