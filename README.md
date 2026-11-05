# UltiCP — Portapapeles Avanzado y Snippets (Windows + Android, Local-First)

Alternativa ligera y privada a **Paste**, **Ditto** y **Alfred Snippets**, diseñada bajo una arquitectura **Local-First** con **Tauri v2 (Rust + React/TypeScript)** para Windows y **Kotlin (`AccessibilityService`) + núcleo compartido** para Android, sincronizados sin nube ajena mediante **Syncthing P2P**.

---

## Características Principales

1. **Historial Categorizado Automáticamente**:
   - **Texto plano** (`plain_text`)
   - **Código** (`code`) con detección automática de lenguaje (`JSON`, `Rust`, `TypeScript`, `JavaScript`, `Python`, `SQL`, `HTML`, `CSS`, `Shell`)
   - **URLs** (`url`)
   - **Imágenes** (`image`) capturadas directamente del portapapeles del sistema y almacenadas por contenido (`SHA-256`)
   - **Fijados (Pinned)** y deduplicación por hash SHA-256 con contador de usos.

2. **Expansión de Snippets y Plantillas**:
   - Abreviaturas personalizables (`;firma`, `;reunion`, `;pr`, `;fecha`, `;uuid`).
   - Variables dinámicas en tiempo real: `{{date}}`, `{{time}}`, `{{datetime}}`, `{{clipboard}}`, `{{uuid}}`.
   - Expansión automática al copiar o mediante el probador en vivo / `AccessibilityService` en Android.

3. **Limpieza Automática de Formato al Pegar**:
   - **Eliminación de parámetros de tracking en URLs**: Limpia `utm_source`, `utm_medium`, `utm_campaign`, `utm_term`, `utm_content`, `fbclid`, `gclid`, `msclkid`, `igshid`, `mc_cid`, `si` (YouTube/Spotify), parámetros de seguimiento de Amazon, etc., tanto en URLs sueltas como dentro de textos largos.
   - **Formateador / Minificador de JSON**: Convierte JSON minificado en JSON indentado legible (2 espacios) o viceversa.
   - **Normalizador de Markdown**: Unifica listas, cabeceras `# Título`, espacios al final de línea y saltos de línea excesivos.

4. **Sincronización P2P sin Conflictos para Syncthing**:
   - SQLite local en modo WAL (`ulticp_local.sqlite3`) para consultas instantáneas.
   - Carpeta de sincronización desacoplada por dispositivo:
     - `syncthing_shared/devices/<device_id>.jsonl` (cada dispositivo escribe **únicamente** en su propio archivo `.jsonl`, evitando bloqueos o archivos `.sync-conflict` de SQLite en Syncthing).
     - `syncthing_shared/blobs/<sha256>.png` (imágenes direccionadas por hash).
   - Resolución automática de conflictos **Last-Write-Wins (LWW)** por marca de tiempo `updated_at` y hash `SHA-256`.

---

## Estructura del Monorepo

- `crates/ulticp-core/`: Librería Rust multiplataforma compartida (Clasificador, Limpiador de URLs/JSON/Markdown, Motor de Snippets, Base de datos SQLite y Motor de Sincronización JSONL para Syncthing).
- `src-tauri/`: Backend de escritorio/móvil Tauri v2 con demonio de escucha del portapapeles (`arboard` + `image`) y comandos IPC.
- `src/`: Interfaz de usuario React + TypeScript + Vite.
- `android/`: Servicio nativo Kotlin (`UltiCpAccessibilityService.kt`, `FormatCleaner.kt`, `SyncthingRepository.kt`) para expansión de snippets en cualquier aplicación de Android y sincronización con la carpeta de Syncthing.

---

## Comandos de Desarrollo y Ejecución

```powershell
# 1. Instalar dependencias del frontend
npm install

# 2. Ejecutar tests unitarios del núcleo en Rust
cargo test -p ulticp-core

# 3. Iniciar la aplicación de escritorio en modo desarrollo (Tauri + Vite)
npm run tauri dev

# 4. Compilar ejecutable e instalador de producción para Windows
npm run tauri build
```

