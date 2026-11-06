# Ficha para Nexus Mods

Todo lo que pide Nexus al crear la página y subir el archivo. La descripción larga está en
[`descripcion.bbcode`](descripcion.bbcode) (Nexus usa BBCode: se pega tal cual en el editor, en
modo BBCode).

## Página

| Campo | Valor |
|---|---|
| Juego | Elden Ring Nightreign |
| Nombre | Relics Reforged |
| Versión | 0.1.0 |
| Autor | Ignacio |
| Categoría | La que más se parezca a "Gameplay" en la lista del juego (no está verificada la lista de categorías de Nightreign) |
| Idioma | English (con sección en español) |
| Contenido para adultos | No |

**Resumen** (Nexus lo limita a unos 350 caracteres; este tiene 265):

> Transfer one effect from a relic you don't need onto one you want to keep, inside Relic Rites. The result is always a relic the game could have rolled. Costs murk and 3 sacrifices. Mouse, keyboard and controller. Works offline and with Seamless Co-op. Requires me3.

**Requisitos** (sección "Requirements" de Nexus):

- me3 (enlace a la página de me3 en Nexus si existe, o a https://me3.help).
- Opcional: Seamless Co-op para Nightreign (enlace a su página en Nexus).

**Permisos** (decisión tuya; lo habitual para un mod con código abierto):

- Subir el mod a otros sitios: pedir permiso.
- Modificar o reutilizar partes: permitido con crédito (el código es MIT).
- Convertir a otros juegos: no aplica.
- Uso en mods pagos o con donaciones: a elección.

## Archivo

| Campo | Valor |
|---|---|
| Archivo | `target/dist/RelicsReforged-v0.1.0.zip` (`powershell -File tools/package.ps1`) |
| Nombre del archivo | Relics Reforged |
| Versión | 0.1.0 |
| Categoría | Main Files |
| Descripción | Unzip the RelicsReforged folder anywhere and double-click RelicsReforged.me3 (offline) or RelicsReforged-Seamless.me3 (copy your SeamlessCoop folder next to it first). Requires me3 0.13+ and game version 1.3.3. |

**Changelog 0.1.0**

- First release: effect transfer between relics in Relic Rites, with preview, double confirmation, forge limits and save backups.
- In-game menu in the style of the game, with search, filter, sorting, icons and sounds from the game.
- Mouse, keyboard and controller (Steam Input) support; the game doesn't receive inputs while the forge is open.
- English and Spanish.
- Offline and Seamless Co-op profiles for me3.

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
- [ ] Sesión de Seamless con un amigo, con y sin el mod del otro lado.
- [ ] Probar en otra PC (otra ruta del juego, sin `data/export`).
- [ ] Pasar la rama a `main` y etiquetar la versión (`v0.1.0`).
- [ ] Sacar las capturas.
