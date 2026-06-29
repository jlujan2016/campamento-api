# 🏕️ Campamento App

Sistema web para control de turnos en campamentos previos a conciertos. Reemplaza el cronograma en Excel con una PWA instalable en iOS y Android que permite gestionar turnos rotativos, check-in/out con GPS, aportes, métricas de transparencia y notificaciones automáticas por Telegram.

---

## 📋 Tabla de contenidos

- [Características](#características)
- [Stack tecnológico](#stack-tecnológico)
- [Arquitectura](#arquitectura)
- [Requisitos previos](#requisitos-previos)
- [Instalación y configuración](#instalación-y-configuración)
- [Variables de entorno](#variables-de-entorno)
- [Migraciones de base de datos](#migraciones-de-base-de-datos)
- [Correr el proyecto](#correr-el-proyecto)
- [API — Endpoints](#api--endpoints)
- [Roles y permisos](#roles-y-permisos)
- [Las 4 métricas de transparencia](#las-4-métricas-de-transparencia)
- [Notificaciones Telegram](#notificaciones-telegram)
- [PWA — Instalación en celular](#pwa--instalación-en-celular)
- [Estructura del proyecto](#estructura-del-proyecto)
- [Próximos pasos](#próximos-pasos)

---

## ✨ Características

- **Multi-evento**: una persona puede participar en varios campamentos en paralelo (ej. The Strokes y Hayley Williams simultáneamente)
- **Cronograma colaborativo**: el admin define franjas horarias con cupos; los participantes eligen sus turnos
- **Enlace temporal público**: se comparte por Telegram/WhatsApp para que alguien se anote sin necesidad de crear cuenta
- **Check-in/out con GPS**: registro de entrada y salida con coordenadas de referencia (no bloqueante) y foto opcional
- **Reemplazos de turno**: parciales o totales, con confirmación cruzada entre las dos partes
- **Turnos extra espontáneos**: alguien puede ir aunque no esté en el cronograma; el admin lo aprueba
- **Retiro de participantes**: libera automáticamente los turnos futuros y notifica al grupo
- **Vacíos sin resolver**: si alguien avisó que llega tarde y nadie cubre, el turno se marca visible para el admin
- **Corrimiento de horario por tardanza**: si alguien llega tarde (pasada la tolerancia configurable), su turno se extiende para completar las horas comprometidas
- **Aportes**: carpa, colchón, comida, transporte, dinero — cada uno con bono de horas configurable
- **4 métricas de transparencia**: horas debidas, horas reales, con tramo final, con aportes
- **Ranking oficial de la fila**: ordenado por métrica 4 (total con aportes)
- **Tramo final**: registro opcional de presencia el día del concierto; bloquea el ranking si no se cumplen las horas mínimas
- **Turno nocturno**: requisito informativo (no bloqueante) configurable por evento
- **Notificaciones Telegram**: grupales (huecos, enlace nuevo) y privadas (recordatorio 1h antes, reemplazo confirmado, aporte aprobado)
- **PWA**: instalable en iOS y Android sin pasar por las stores

---

## 🛠️ Stack tecnológico

### Backend
| Tecnología | Rol |
|---|---|
| **Rust** | Lenguaje del backend |
| **Axum 0.7** | Framework web HTTP |
| **Tokio** | Motor asíncrono |
| **SQLx 0.7** | Queries SQL con verificación en tiempo de compilación |
| **PostgreSQL 16** | Base de datos principal |
| **PostGIS** | Extensión para datos de geolocalización |
| **Argon2** | Hash seguro de contraseñas |
| **JWT (jsonwebtoken)** | Autenticación stateless |
| **tower-http** | Middleware CORS |
| **reqwest** | Cliente HTTP para la API de Telegram |
| **Docker** | Contenedor de la base de datos |

### Frontend
| Tecnología | Rol |
|---|---|
| **React 18 + TypeScript** | Framework UI |
| **Vite** | Bundler y servidor de desarrollo |
| **Tailwind CSS v4** | Estilos |
| **React Router v6** | Navegación |
| **TanStack Query** | Manejo de estado del servidor |
| **date-fns** | Formateo de fechas |
| **lucide-react** | Iconos |

---

## 🏗️ Arquitectura

```
┌─────────────────────────────────────────────────────┐
│                   Cliente (PWA)                      │
│              React + TypeScript + Vite               │
│         Móvil (iOS/Android) y Web (escritorio)       │
└───────────────────┬─────────────────────────────────┘
                    │ HTTP/JSON (REST API)
                    │ CORS configurado
┌───────────────────▼─────────────────────────────────┐
│                Backend (Rust / Axum)                  │
│                                                       │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │   Rutas     │  │  Middleware  │  │   Worker    │ │
│  │  (handlers) │  │  (JWT auth)  │  │  Telegram   │ │
│  └──────┬──────┘  └──────────────┘  └──────┬──────┘ │
│         │                                   │        │
│  ┌──────▼───────────────────────────────────▼──────┐ │
│  │                    SQLx                          │ │
│  └──────────────────────┬───────────────────────────┘ │
└─────────────────────────┼───────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────┐
│           PostgreSQL 16 + PostGIS (Docker)            │
│                                                       │
│  users · events · event_members · schedule_slots      │
│  slot_signups · shifts · checkins · contributions     │
│  notifications · telegram_links · final_checkpoints   │
└─────────────────────────────────────────────────────┘
```

---

## 📦 Requisitos previos

- **Rust** (stable) — instalar desde [rustup.rs](https://rustup.rs)
- **Docker** y **Docker Compose** — para la base de datos
- **Node.js 18+** y **npm** — para el frontend
- **sqlx-cli** — para las migraciones:
  ```bash
  cargo install sqlx-cli --no-default-features --features rustls,postgres
  ```

---

## ⚙️ Instalación y configuración

### 1. Clonar los repositorios

```bash
git clone https://github.com/jlujan2016/campamento-api.git
git clone https://github.com/jlujan2016/campamento-web.git
```

### 2. Configurar el backend

```bash
cd campamento-api
cp .env.example .env
# Editar .env con tus valores reales
```

### 3. Levantar la base de datos

```bash
docker compose up -d
# Verificar que está corriendo
docker compose ps
# Verificar que PostgreSQL está listo
docker compose logs db | tail -5
```

### 4. Aplicar las migraciones

```bash
sqlx migrate run
```

### 5. Configurar el frontend

```bash
cd campamento-web
cp .env.example .env
# Editar .env con la IP del backend
npm install
```

---

## 🔐 Variables de entorno

### Backend (`campamento-api/.env`)

```env
# Base de datos
DB_USER=campamento
DB_PASSWORD=tu_password_seguro
DB_NAME=campamento_db
DB_PORT=5432

# URL completa para SQLx
DATABASE_URL=postgres://campamento:tu_password@localhost:5432/campamento_db

# Autenticación JWT
JWT_SECRET=clave_larga_aleatoria_minimo_32_caracteres

# Servidor
PORT=3000

# Telegram (opcional — si no se configura, las notificaciones se desactivan)
TELEGRAM_BOT_TOKEN=tu_token_de_botfather
```

### Frontend (`campamento-web/.env`)

```env
# URL del backend (usar IP de red local para acceso desde celular)
VITE_API_URL=http://192.168.1.XXX:3000
```

---

## 🗄️ Migraciones de base de datos

El proyecto usa SQLx con migraciones versionadas en la carpeta `migrations/`. Las tablas principales son:

```
users                 → usuarios (registrados e invitados)
events                → campamentos/conciertos
event_members         → relación usuario↔evento con rol
schedule_slots        → franjas horarias del cronograma
slot_signups          → inscripciones a slots
shifts                → turnos asignados (scheduled o extra)
shift_replacements    → solicitudes de reemplazo
checkins              → registros de entrada/salida con GPS
contributions         → aportes (carpa, colchón, dinero, etc.)
contribution_types    → tabla de equivalencias de aportes por evento
final_checkpoints     → tramo final antes del concierto
final_attendance      → presencia en el tramo final
notifications         → cola de notificaciones para Telegram
telegram_links        → vínculos entre usuarios/eventos y chats de Telegram
schedule_links        → enlaces temporales públicos para el cronograma
```

Para crear una nueva migración:
```bash
sqlx migrate add nombre_descriptivo
# Editar el archivo generado en migrations/
sqlx migrate run
```

---

## 🚀 Correr el proyecto

### Backend

```bash
cd campamento-api
cargo run
# Salida esperada:
# ✅ Conexión a la base de datos establecida
# ✅ Migraciones aplicadas
# 🤖 Worker de Telegram iniciado
# 🚀 Servidor corriendo en http://0.0.0.0:3000
```

### Frontend

```bash
cd campamento-web
npm run dev
# Abrí http://localhost:5173 en el navegador
# O http://192.168.1.XXX:5173 desde el celular en la misma red
```

---

## 📡 API — Endpoints

### Autenticación (público)
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/auth/register` | Crear cuenta nueva |
| POST | `/auth/login` | Iniciar sesión, recibir JWT |
| GET | `/auth/me` | Ver mis datos (requiere JWT) |

### Eventos (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/events` | Listar eventos activos |
| POST | `/events` | Crear evento (solo super admin) |
| GET | `/events/:id` | Ver un evento |
| PUT | `/events/:id` | Editar evento (admin del evento) |
| POST | `/events/:id/join` | Unirse a un evento |
| GET | `/events/:id/members` | Ver miembros del evento |
| POST | `/events/:id/members/:uid/withdraw` | Retirar participante (admin) |

### Cronograma (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/events/:id/slots` | Listar slots con disponibilidad |
| POST | `/events/:id/slots` | Crear slot (admin) |
| GET | `/events/:id/slots/:sid/signups` | Ver inscriptos en un slot |
| POST | `/events/:id/signup-slots` | Anotarse en uno o varios slots |
| POST | `/events/:id/schedule-link` | Generar enlace temporal |
| GET | `/schedule/:token` | Ver cronograma público (sin cuenta) |
| POST | `/schedule/:token/signup` | Anotarse sin cuenta |

### Turnos (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/events/:id/shifts` | Mis turnos en un evento |
| POST | `/events/:id/shifts` | Crear turno extra espontáneo |
| GET | `/events/:id/shifts/active` | Ver quién está presente ahora |
| GET | `/events/:id/shifts/all` | Todos los turnos del evento (admin) |
| GET | `/events/:id/shifts/gaps` | Turnos con vacío sin resolver (admin) |
| POST | `/shifts/:id/checkin` | Hacer check-in (con GPS opcional) |
| POST | `/shifts/:id/checkout` | Hacer check-out (con GPS opcional) |
| POST | `/shifts/:id/mark-gap` | Marcar turno como vacío |

### Reemplazos (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/shifts/:id/replacement` | Solicitar reemplazo (total o parcial) |
| PUT | `/shifts/:id/replacement/:rid` | Confirmar o rechazar reemplazo |

### Aportes (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/events/:id/contribution-types` | Listar tipos de aporte |
| POST | `/events/:id/contribution-types` | Crear tipo de aporte (admin) |
| POST | `/events/:id/contributions` | Registrar un aporte |
| PUT | `/contributions/:id/approve` | Aprobar/rechazar aporte (admin) |

### Tramo final (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/events/:id/final-checkpoint` | Crear tramo final (admin) |
| POST | `/events/:id/final-checkpoint/attend` | Registrar presencia |

### Métricas y ranking (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/events/:id/metrics` | Las 4 métricas de cada persona |
| GET | `/events/:id/ranking` | Orden oficial de la fila |

### Telegram (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/events/:id/telegram/group` | Vincular grupo de Telegram al evento |
| POST | `/telegram/link-account` | Vincular cuenta personal de Telegram |

---

## 👥 Roles y permisos

```
Super admin
├── Crea eventos y asigna admins
├── Ve todos los eventos y datos
└── Tiene todos los permisos de admin de evento

Admin de evento
├── Define cronograma (slots, cupos, duración mín/máx)
├── Genera enlace temporal para el cronograma
├── Aprueba turnos extra, aportes y slots propuestos
├── Retira participantes (libera sus turnos automáticamente)
├── Define tramo final y rango horario nocturno
├── Ve métricas y ranking de todos los participantes
└── Vincula el grupo de Telegram al evento

Participante
├── Se anota en slots del cronograma
├── Solicita turnos extra espontáneos
├── Hace check-in/out con GPS y foto opcional
├── Registra aportes (pendientes de aprobación)
├── Solicita y confirma reemplazos
└── Ve sus propias métricas y el ranking general

Invitado (sin cuenta)
└── Se anota en el cronograma via enlace temporal
    (solo nombre y teléfono, puede completar cuenta después)
```

---

## 📊 Las 4 métricas de transparencia

Para cada persona en cada evento el sistema calcula y muestra:

| # | Métrica | Qué incluye | Uso |
|---|---|---|---|
| 1 | **Horas debidas** | Suma de duración de shifts asignados en el cronograma | Referencia |
| 2 | **Horas reales** | Horas efectivas medidas por check-in/out | Verifica el mínimo exigido |
| 3 | **Reales + tramo final** | Métrica 2 + horas del tramo final (si asistió) | Transparencia |
| 4 | **Total con aportes** | Métrica 3 + bono de horas por aportes aprobados | **Orden oficial de la fila** |

> **Regla de habilitación**: si el evento tiene configurado un mínimo de horas, solo pueden registrar presencia en el tramo final y aparecer en el ranking quienes hayan cumplido ese mínimo en la métrica 2 (horas reales, sin contar aportes). Esto evita que alguien "compre" su lugar solo aportando cosas sin hacer turnos reales.

---

## 🤖 Notificaciones Telegram

### Configuración del bot
1. Buscar `@BotFather` en Telegram
2. Enviar `/newbot` y seguir los pasos
3. Copiar el token al `.env` como `TELEGRAM_BOT_TOKEN`
4. Agregar el bot al grupo del campamento

### Obtener el chat_id del grupo
```
https://api.telegram.org/bot{TOKEN}/getUpdates
```
Buscar el campo `"chat" → "id"` (número negativo para grupos).

### Vincular via API
```bash
# Vincular grupo al evento
POST /events/:id/telegram/group
{ "telegram_chat_id": "-1001234567890" }

# Vincular cuenta personal
POST /telegram/link-account
{ "telegram_chat_id": "123456789" }
```

### Eventos que disparan notificaciones

| Evento | Destino | Cuándo |
|---|---|---|
| Hueco liberado por retiro | Grupo | Admin retira a un participante |
| Nuevo enlace de cronograma | Grupo | Admin genera un schedule_link |
| Vacío sin resolver | Grupo | Turno pasa a `gap_unresolved` |
| Turno extra aprobado | Privado | Admin aprueba el turno extra |
| Aporte aprobado | Privado | Admin aprueba la contribución |
| Reemplazo confirmado | Privado (ambos) | `shift_replacement` pasa a confirmed |
| Recordatorio de turno | Privado | 1 hora antes del turno programado |

El worker procesa la cola cada 30 segundos — si Telegram está caído, reintenta en el siguiente ciclo sin bloquear el servidor.

---

## 📱 PWA — Instalación en celular

### Android (Chrome)
1. Abrí `http://IP_DEL_SERVIDOR:5173` en Chrome
2. Tocá los tres puntos → "Agregar a pantalla de inicio"
3. Confirmá — la app aparece como icono en el home

### iOS (Safari)
1. Abrí `http://IP_DEL_SERVIDOR:5173` en Safari
2. Tocá el botón de compartir (cuadrado con flecha)
3. "Agregar a pantalla de inicio"
4. Confirmá

> Para acceso desde el celular, asegurate de que el celular y la PC que corre el servidor estén en la **misma red WiFi**.

---

## 📁 Estructura del proyecto

### Backend (`campamento-api/`)

```
campamento-api/
├── migrations/                  # Migraciones SQL versionadas
│   ├── 001_create_initial_schema.sql
│   └── 002_fix_numeric_to_float.sql
├── src/
│   ├── main.rs                  # Punto de entrada, configura servidor y worker
│   ├── config.rs                # Lee variables de entorno
│   ├── db.rs                    # Pool de conexiones a PostgreSQL
│   ├── errors.rs                # Tipos de error y respuestas HTTP
│   ├── auth.rs                  # JWT y hash de contraseñas (Argon2)
│   ├── telegram.rs              # Cliente de la API de Telegram
│   ├── worker.rs                # Worker de notificaciones (corre cada 30s)
│   ├── models/
│   │   ├── user.rs
│   │   ├── event.rs
│   │   ├── schedule.rs
│   │   ├── shift.rs
│   │   ├── replacement.rs
│   │   ├── contribution.rs
│   │   └── metrics.rs
│   └── routes/
│       ├── mod.rs               # Router principal + CORS
│       ├── health.rs
│       ├── auth.rs
│       ├── events.rs
│       ├── schedule.rs
│       ├── shifts.rs
│       ├── replacements.rs
│       ├── contributions.rs
│       ├── metrics.rs
│       └── telegram.rs
├── Cargo.toml
├── docker-compose.yml
├── .env.example
└── .gitignore
```

### Frontend (`campamento-web/`)

```
campamento-web/
├── public/
│   └── manifest.json            # Configuración PWA
├── src/
│   ├── api/
│   │   ├── client.ts            # Cliente HTTP base con JWT automático
│   │   ├── auth.ts
│   │   ├── events.ts
│   │   └── shifts.ts
│   ├── components/
│   │   ├── BottomNav.tsx        # Navegación inferior (móvil)
│   │   ├── CheckinButton.tsx    # Botón de check-in/out con GPS
│   │   ├── MetricsCard.tsx      # Tarjeta de las 4 métricas
│   │   └── ShiftCard.tsx        # Tarjeta de turno individual
│   ├── hooks/
│   │   └── useAuth.ts           # Context y hook de autenticación
│   ├── pages/
│   │   ├── LoginPage.tsx
│   │   ├── RegisterPage.tsx
│   │   ├── DashboardPage.tsx    # Dashboard del participante
│   │   ├── AdminPage.tsx        # Panel del admin (ranking + miembros)
│   │   └── ScheduleLinkPage.tsx # Vista pública del enlace temporal
│   ├── types/
│   │   └── index.ts             # Tipos TypeScript de la API
│   ├── main.tsx                 # Punto de entrada, rutas
│   └── index.css                # Tailwind + componentes globales
├── index.html                   # Configura PWA meta tags
├── vite.config.ts
├── .env.example
└── .gitignore
```

---

## 🗺️ Próximos pasos

- [ ] Pantalla de aprobaciones para el admin (turnos extra, aportes pendientes)
- [ ] Vista del cronograma completo del evento
- [ ] Subida de foto en check-in (integración con almacenamiento S3-compatible)
- [ ] Adaptación del layout para escritorio (`md:` breakpoints en Tailwind)
- [ ] Deploy a producción (backend en Fly.io o Railway, frontend en Vercel)
- [ ] Integración con WhatsApp Business API (como alternativa a Telegram)
- [ ] Modo offline básico (service worker para ver datos sin conexión)
- [ ] Tests unitarios del backend (Rust tiene un framework de testing integrado)

---

## 📚 Recursos para aprender más

| Tema | Recurso |
|---|---|
| Rust fundamentals | [The Rust Book](https://doc.rust-lang.org/book/) (gratuito) |
| Async Rust | [Async Rust Book](https://rust-lang.github.io/async-book/) (gratuito) |
| Rust en producción | *Zero to Production in Rust* (libro) |
| PostgreSQL | [postgresql.org/docs](https://www.postgresql.org/docs/) |
| React + TypeScript | [react.dev](https://react.dev) |
| PWA | [web.dev/progressive-web-apps](https://web.dev/progressive-web-apps/) |
| JWT | [jwt.io/introduction](https://jwt.io/introduction/) |
| Docker | [docs.docker.com/get-started](https://docs.docker.com/get-started/) |

---

## 📄 Licencia

MIT
