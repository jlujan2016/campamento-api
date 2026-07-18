# 🚀 DESPLIEGUE — ¡Callate y baila!

Guía completa para correr la aplicación en distintos escenarios.
La app tiene dos repos que trabajan juntos:

- **`campamento-api`** — Backend en Rust (Axum) + PostgreSQL en Docker
- **`campamento-web`** — Frontend en React (Vite) compilado como PWA

En producción, el backend sirve tanto la API como el frontend compilado.
**Todo sale por un solo puerto (8090).**

---

## Requisitos previos

| Herramienta | Versión mínima | Para qué se usa |
|---|---|---|
| [Rust](https://rustup.rs) | stable | Compilar el backend |
| [Docker Desktop](https://docker.com) | cualquiera | Correr PostgreSQL |
| [Node.js](https://nodejs.org) | 18+ | Compilar el frontend |
| [sqlx-cli](https://github.com/launchbadge/sqlx) | 0.7+ | Migraciones de DB |
| [Git](https://git-scm.com) | cualquiera | Clonar el proyecto |

Instalar sqlx-cli:
```bash
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

---

## A. PC nueva (desde cero)

Estos pasos instalan y corren todo en una PC que nunca tuvo el proyecto.

### 1. Clonar los repositorios

```bash
git clone https://github.com/tuusuario/campamento-api.git
git clone https://github.com/tuusuario/campamento-web.git
```

### 2. Configurar el backend

```bash
cd campamento-api
cp .env.example .env   # Windows: copy .env.example .env
```

Editá `.env` con estos valores mínimos:

```env
DB_USER=campamento
DB_PASSWORD=una_password_segura
DB_NAME=campamento_db
DB_PORT_HOST=5433
DATABASE_URL=postgres://campamento:una_password_segura@localhost:5433/campamento_db

APP_PORT=8090
APP_HOST=0.0.0.0

JWT_SECRET=generá_una_clave_con_openssl_rand_hex_32
CORS_ORIGINS=http://localhost:5174
```

### 3. Levantar la base de datos

```bash
docker compose up -d

# Verificar que está corriendo y saludable
docker compose ps
# Esperar hasta ver: Up (healthy)
```

### 4. Configurar y compilar el frontend

```bash
cd ../campamento-web
cp .env.example .env   # Windows: copy .env.example .env
```

El `.env` del frontend para desarrollo:

```env
VITE_API_URL=/api
VITE_PORT=5174
VITE_BACKEND_URL=http://localhost:8090
```

Instalar dependencias:

```bash
npm install
```

### 5. Modo desarrollo (front y back separados)

**Terminal 1 — Backend:**
```bash
cd campamento-api
cargo run
# ✅ Conexión a la base de datos establecida
# 🚀 Servidor corriendo en http://0.0.0.0:8090
```

**Terminal 2 — Frontend:**
```bash
cd campamento-web
npm run dev -- --host
# Local:   http://localhost:5174/
# Network: http://192.168.X.X:5174/
```

Abrí `http://localhost:5174` en el navegador.

### 6. Modo producción (un solo puerto)

```bash
# Paso 1: compilar el frontend
cd campamento-web
npm run build
# Genera la carpeta dist/

# Paso 2: activar FRONTEND_DIST en el backend
# Editá campamento-api/.env y agregá:
# Linux/Mac:
FRONTEND_DIST=../campamento-web/dist
# Windows:
FRONTEND_DIST=C:/Users/TU_USUARIO/campamento-web/dist

# Y dejá CORS_ORIGINS vacío (mismo origen):
CORS_ORIGINS=

# Paso 3: correr el backend
cd campamento-api
cargo run
# 📁 Sirviendo frontend desde: ../campamento-web/dist
# 🌐 Modo producción — sirviendo frontend + API desde mismo origen
```

Abrí `http://localhost:8090` — carga el frontend directamente desde Axum.

---

## B. Junto a otra app en la misma PC

Si ya tenés otra app que usa los puertos 8080, 5173 o 5432,
cambiá estas variables en `.env` para evitar conflictos:

### `campamento-api/.env`

```env
# Puerto del backend — cambiá si 8090 está ocupado
APP_PORT=8091

# Puerto de PostgreSQL en el host — cambiá si 5433 está ocupado
DB_PORT_HOST=5434
DATABASE_URL=postgres://campamento:PASSWORD@localhost:5434/campamento_db

# CORS — ajustá al puerto de Vite que uses
CORS_ORIGINS=http://localhost:5175
```

### `campamento-web/.env`

```env
# Puerto de Vite — cambiá si 5174 está ocupado
VITE_PORT=5175

# URL del backend con el nuevo puerto
VITE_BACKEND_URL=http://localhost:8091
```

### Verificar que no hay conflictos

```bash
# Windows — ver qué puertos están en uso
netstat -ano | findstr ":8090"
netstat -ano | findstr ":5433"

# Linux
ss -tlnp | grep -E "8090|5433"
```

Si un puerto devuelve resultados, está ocupado — elegí otro.

---

## C. Publicar en Cloudflare Tunnel (appconcert.online)

> ⚠️ **LEER ANTES DE EJECUTAR**
>
> Esta sección asume que:
> - El dominio `appconcert.online` ya está delegado a Cloudflare
> - El conector `cloudflared` ya corre como servicio en la PC
> - El túnel ya existe y está activo
>
> **No se crea ni modifica ninguna configuración de Cloudflare en este paso —
> solo se agrega una ruta nueva al túnel existente.**

### Paso previo obligatorio — limpiar DNS

Antes de agregar la ruta en el túnel, hay que eliminar el registro A
del dominio raíz en Cloudflare para que no choque con el túnel:

1. Entrá a [dash.cloudflare.com](https://dash.cloudflare.com)
2. Seleccioná el dominio `appconcert.online`
3. Andá a **DNS → Records**
4. Buscá el registro de tipo **A** que apunta al dominio raíz
   (el que dice `@` o `appconcert.online` en el campo Name)
5. **Eliminalo** — este es el registro de estacionamiento de GoDaddy
   que choca con el túnel de Cloudflare
6. Guardá los cambios

> Si no eliminás este registro, la ruta del túnel no va a funcionar
> porque el registro A tiene prioridad sobre el túnel.

### Configurar la app para producción

En `campamento-api/.env`, ajustá para producción con Cloudflare:

```env
# El tunnel de Cloudflare conecta desde afuera al localhost
# APP_HOST=127.0.0.1 es más seguro — solo acepta conexiones locales
APP_HOST=127.0.0.1
APP_PORT=8090

# En producción, mismo origen — CORS vacío
CORS_ORIGINS=

# Ruta al frontend compilado
# Linux:
FRONTEND_DIST=/home/TU_USUARIO/campamento-web/dist
# Windows:
FRONTEND_DIST=C:/Users/TU_USUARIO/campamento-web/dist

# JWT_SECRET debe ser una clave larga y segura en producción
# Generá con: openssl rand -hex 32
JWT_SECRET=clave_larga_generada_con_openssl
```

### Compilar el frontend para producción

```bash
cd campamento-web
npm run build
# Genera dist/ con el frontend optimizado
```

### Correr el backend

```bash
cd campamento-api
cargo run
# 🚀 Servidor corriendo en http://127.0.0.1:8090
# 🌐 Modo producción — sirviendo frontend + API desde mismo origen
```

### Agregar la ruta en el túnel de Cloudflare

1. Entrá a [dash.cloudflare.com](https://dash.cloudflare.com)
2. Andá a **Zero Trust → Networks → Tunnels**
3. Hacé clic en tu túnel existente → **Edit**
4. Andá a la pestaña **Public Hostname**
5. Hacé clic en **Add a public hostname**
6. Completá exactamente así:

```
Subdomain:  (dejar VACÍO — es el dominio raíz)
Domain:     appconcert.online
Path:       (dejar vacío)
Service:    http://localhost:8090
```

7. Guardá

### Verificar

Después de guardar, esperá 1-2 minutos y abrí:

```
https://appconcert.online
```

Deberías ver la pantalla de login de "¡Callate y baila!".

Verificá en el navegador (F12 → Network) que:
- Las requests van a `https://appconcert.online/api/...`
- Hay un candado HTTPS (Cloudflare lo agrega automáticamente)
- No hay errores de CORS

> 💡 **HTTPS automático**: Cloudflare agrega HTTPS sin que tengas que
> configurar certificados. Esto resuelve automáticamente los problemas
> de GPS e iOS que teníamos en desarrollo con HTTP.

---

## Comandos de mantenimiento

### Base de datos

```bash
# Ver estado del contenedor
docker compose ps

# Ver logs de PostgreSQL
docker compose logs db --tail=50

# Conectarse a la base de datos
docker exec -it campamento-api-db-1 psql -U campamento -d campamento_db

# Hacer backup
docker exec campamento-api-db-1 \
  pg_dump -U campamento campamento_db > backup_$(date +%Y%m%d_%H%M%S).sql

# Restaurar backup
cat backup_20240101_120000.sql | \
  docker exec -i campamento-api-db-1 psql -U campamento -d campamento_db
```

### Actualizar la app

```bash
# 1. Traer los cambios
cd campamento-api && git pull
cd ../campamento-web && git pull

# 2. Recompilar el frontend si hubo cambios
cd campamento-web && npm run build

# 3. Reiniciar el backend
cd campamento-api && cargo run
# (las migraciones nuevas se aplican automáticamente al arrancar)
```

### Logs del backend

```bash
# Ver logs en tiempo real (Linux)
RUST_LOG=debug cargo run 2>&1 | tee app.log

# Windows
$env:RUST_LOG="debug"; cargo run
```

---

## Troubleshooting

### La app no carga en http://localhost:8090

```bash
# Verificar que el backend está corriendo
curl http://localhost:8090/api/health
# Debe responder: {"status":"ok","database":"conectada"}

# Verificar que FRONTEND_DIST está configurado en .env
# y que la carpeta dist/ existe
ls campamento-web/dist/index.html
```

### Error de conexión a la base de datos

```bash
# Verificar que Docker está corriendo
docker compose ps

# Verificar que el puerto coincide con DATABASE_URL
# Si DB_PORT_HOST=5433, DATABASE_URL debe tener @localhost:5433

# Probar conexión directa
docker exec campamento-api-db-1 pg_isready -U campamento
```

### El túnel de Cloudflare no conecta

```bash
# Verificar que cloudflared está corriendo como servicio
# Linux:
systemctl status cloudflared

# Windows:
Get-Service cloudflared

# Verificar que el backend escucha en 127.0.0.1:8090
# (no en 0.0.0.0 si usás APP_HOST=127.0.0.1)
netstat -an | grep 8090   # Linux
netstat -ano | findstr 8090  # Windows
```

### GPS no funciona en iOS

En desarrollo con HTTP (IP local), iOS bloquea el GPS.
**En producción con HTTPS (Cloudflare), el GPS funciona automáticamente.**

Si necesitás GPS en desarrollo, usá:
```bash
# ngrok como alternativa temporal para HTTPS local
ngrok http 8090
```

---

## Variables de entorno — referencia rápida

| Variable | Dónde | Default | Descripción |
|---|---|---|---|
| `DATABASE_URL` | backend | — | URL completa de PostgreSQL |
| `APP_PORT` | backend | 8090 | Puerto del servidor |
| `APP_HOST` | backend | 0.0.0.0 | Host del servidor |
| `JWT_SECRET` | backend | — | Clave para firmar JWT |
| `CORS_ORIGINS` | backend | vacío | Orígenes permitidos |
| `FRONTEND_DIST` | backend | vacío | Ruta al dist/ del frontend |
| `TELEGRAM_BOT_TOKEN` | backend | vacío | Token del bot |
| `DB_PORT_HOST` | docker | 5433 | Puerto de Postgres en el host |
| `VITE_API_URL` | frontend | /api | URL base de la API |
| `VITE_PORT` | frontend | 5174 | Puerto de Vite dev |
| `VITE_BACKEND_URL` | frontend | localhost:8090 | Backend para el proxy |
