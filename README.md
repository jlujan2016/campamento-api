# 🏕️ Campamento App — Backend

Sistema web para control de turnos en campamentos previos a conciertos. Reemplaza el cronograma en Excel con una PWA instalable en iOS y Android que permite gestionar turnos rotativos, check-in/out con GPS, aportes, métricas de transparencia y notificaciones automáticas por Telegram.

**En producción en [https://appconcert.online](https://appconcert.online)** vía Cloudflare Tunnel — ver [`DESPLIEGUE.md`](./DESPLIEGUE.md) para la guía completa de despliegue (PC nueva, junto a otra app, o publicar en Cloudflare).

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
- [Notificaciones Telegram — guía completa](#-notificaciones-telegram--guía-completa)
- [PWA — Instalación en celular](#pwa--instalación-en-celular)
- [Estructura del proyecto](#estructura-del-proyecto)
- [Próximos pasos](#próximos-pasos)

---

## ✨ Características

- **Multi-evento**: una persona puede participar en varios campamentos en paralelo
- **Cronograma colaborativo**: el admin define franjas horarias con cupos; los participantes eligen sus turnos y ven quién más está anotado
- **Enlace temporal público**: se comparte por Telegram/WhatsApp para que alguien se anote creando cuenta completa (con auto-login) o solo como invitado
- **Check-in/out con GPS**: registro de entrada y salida con coordenadas del dispositivo del usuario (no bloqueante)
- **Reemplazos de turno**: parciales o totales, con confirmación cruzada entre las dos partes
- **Turnos extra espontáneos**: alguien puede ir aunque no esté en el cronograma; el admin lo aprueba
- **Retiro de participantes**: libera automáticamente los turnos futuros y notifica al grupo de Telegram
- **Agregar miembro directo**: el admin puede sumar a un usuario existente al evento sin pasar por el enlace
- **Corrimiento de horario por tardanza**: si alguien llega tarde (pasada la tolerancia configurable), su turno se extiende para completar las horas comprometidas
- **Aportes**: carpa, colchón, comida, transporte, dinero — cada uno con bono de horas configurable
- **4 métricas de transparencia**: horas debidas, horas reales, con tramo final, con aportes
- **Ranking oficial de la fila**: visible para todos los miembros, ordenado por métrica 4 (total con aportes)
- **Gestión de usuarios**: el super admin crea, busca, bloquea y elimina usuarios; promueve o degrada admins de evento
- **Notificaciones Telegram**: grupales (huecos liberados, enlace nuevo) y privadas (recordatorio 1h antes, reemplazo confirmado, aporte aprobado) — con un comando en el bot para obtener el Chat ID sin tocar JSON
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
| **tower-http** | CORS + servir el frontend compilado en producción |
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

**Desarrollo** — frontend y backend como procesos separados, unidos por el proxy de Vite:

```
┌─────────────────────────┐        ┌──────────────────────────┐
│   Vite dev server :5174  │ ──/api→│   Axum backend :8090      │
│   (React + TS)           │  proxy │   API + worker Telegram   │
└─────────────────────────┘        └────────────┬─────────────┘
                                                  │
                                    ┌─────────────▼─────────────┐
                                    │ PostgreSQL 16 + PostGIS    │
                                    │ (Docker)                   │
                                    └────────────────────────────┘
```

**Producción** — un solo proceso, Axum sirve todo por HTTPS vía Cloudflare Tunnel:

```
┌────────────────────────────────────────────────────┐
│         https://appconcert.online (Cloudflare)       │
└───────────────────────┬──────────────────────────────┘
                         │ túnel HTTPS
┌───────────────────────▼──────────────────────────────┐
│              Axum backend :8090                       │
│   /api/*  → API          /*  → frontend compilado      │
│                    Worker Telegram (cada 30s)          │
└───────────────────────┬──────────────────────────────┘
                         │
┌───────────────────────▼──────────────────────────────┐
│           PostgreSQL 16 + PostGIS (Docker)             │
│  users · events · event_members · schedule_slots       │
│  slot_signups · shifts · checkins · contributions       │
│  notifications · telegram_links · schedule_links        │
└────────────────────────────────────────────────────────┘
```

Ver [`DESPLIEGUE.md`](./DESPLIEGUE.md) para el detalle de cada escenario y las variables que cambian entre desarrollo y producción.

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
cp .env.example .env    # Windows: copy .env.example .env
# Editar .env con tus valores reales
```

### 3. Levantar la base de datos

```bash
docker compose up -d
docker compose ps
# Esperar hasta ver: Up (healthy)
```

### 4. Correr el backend

```bash
cargo run
# Las migraciones se aplican automáticamente al arrancar
```

### 5. Configurar el frontend

```bash
cd ../campamento-web
cp .env.example .env
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
DB_PORT_HOST=5433

DATABASE_URL=postgres://campamento:tu_password@localhost:5433/campamento_db

# Servidor
APP_PORT=8090
APP_HOST=0.0.0.0

# Autenticación JWT — generar con: openssl rand -hex 32
JWT_SECRET=clave_larga_aleatoria_minimo_32_caracteres

# CORS — vacío en producción (mismo origen), con valores en desarrollo
CORS_ORIGINS=http://localhost:5174

# Frontend compilado — vacío en desarrollo, con ruta en producción
# FRONTEND_DIST=../campamento-web/dist

# Telegram (opcional — si no se configura, las notificaciones se desactivan)
TELEGRAM_BOT_TOKEN=tu_token_de_botfather
```

### Frontend (`campamento-web/.env`)

```env
VITE_API_URL=/api
VITE_PORT=5174
VITE_BACKEND_URL=http://localhost:8090
```

> 📖 Tabla completa de variables y qué cambia entre desarrollo/producción en [`DESPLIEGUE.md`](./DESPLIEGUE.md#variables-de-entorno--referencia-rápida).

---

## 🗄️ Migraciones de base de datos

El proyecto usa SQLx con migraciones versionadas en la carpeta `migrations/`. Las tablas principales son:

```
users                 → usuarios (registrados e invitados, incluye is_blocked)
events                → campamentos/conciertos
event_members         → relación usuario↔evento con rol (participant/admin) y estado
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
sqlx migrate run
```

---

## 🚀 Correr el proyecto

### Backend

```bash
cd campamento-api
cargo run
# ✅ Conexión a la base de datos establecida
# ✅ Migraciones aplicadas
# 🤖 Worker de Telegram iniciado
# 🚀 Servidor corriendo en http://0.0.0.0:8090
```

### Frontend

```bash
cd campamento-web
npm run dev
# Local:   http://localhost:5174
# Network: http://192.168.X.X:5174
```

---

## 📡 API — Endpoints

### Autenticación (público)
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/api/auth/register` | Crear cuenta nueva |
| POST | `/api/auth/login` | Iniciar sesión (rechaza usuarios bloqueados) |
| GET | `/api/auth/me` | Ver mis datos (requiere JWT) |

### Eventos (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/events` | Listar eventos — filtra por membresía (super admin ve todos) |
| POST | `/api/events` | Crear evento (solo super admin) |
| GET | `/api/events/:id` | Ver un evento |
| PUT | `/api/events/:id` | Editar evento (admin del evento) |
| POST | `/api/events/:id/join` | Unirse a un evento |
| GET | `/api/events/:id/members` | Ver miembros del evento |
| POST | `/api/events/:id/members/add` | Agregar usuario existente al evento (admin) |
| POST | `/api/events/:id/members/:uid/withdraw` | Retirar participante (admin) |

### Cronograma (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/events/:id/slots` | Listar slots con disponibilidad |
| POST | `/api/events/:id/slots` | Crear slot (admin) |
| GET | `/api/events/:id/slots/:sid/signups` | Ver inscriptos en un slot (cualquier miembro) |
| POST | `/api/events/:id/signup-slots` | Anotarse en uno o varios slots |
| POST | `/api/events/:id/schedule-link` | Generar enlace temporal (admin) |
| GET | `/api/schedule/:token` | Ver cronograma público (sin cuenta) |
| POST | `/api/schedule/:token/signup` | Anotarse creando cuenta o como invitado |

### Turnos (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/events/:id/shifts` | Mis turnos en un evento |
| POST | `/api/events/:id/shifts` | Crear turno extra espontáneo |
| GET | `/api/events/:id/shifts/active` | Ver quién está presente ahora |
| GET | `/api/events/:id/shifts/all` | Todos los turnos del evento (admin) |
| GET | `/api/events/:id/shifts/gaps` | Turnos con vacío sin resolver (admin) |
| POST | `/api/shifts/:id/checkin` | Hacer check-in (con GPS opcional) |
| POST | `/api/shifts/:id/checkout` | Hacer check-out (con GPS opcional) |
| POST | `/api/shifts/:id/mark-gap` | Marcar turno como vacío |

### Reemplazos (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/api/shifts/:id/replacement` | Solicitar reemplazo (total o parcial) |
| PUT | `/api/shifts/:id/replacement/:rid` | Confirmar o rechazar reemplazo |

### Aportes (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/events/:id/contribution-types` | Listar tipos de aporte |
| POST | `/api/events/:id/contribution-types` | Crear tipo de aporte (admin) |
| POST | `/api/events/:id/contributions` | Registrar un aporte |
| PUT | `/api/contributions/:id/approve` | Aprobar/rechazar aporte (admin) |

### Tramo final (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| POST | `/api/events/:id/final-checkpoint` | Crear tramo final (admin) |
| POST | `/api/events/:id/final-checkpoint/attend` | Registrar presencia |

### Métricas y ranking (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/events/:id/metrics` | Las 4 métricas de cada persona |
| GET | `/api/events/:id/ranking` | Orden oficial de la fila — visible para todos los miembros |

### Gestión de usuarios (requiere JWT, super admin)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/users` | Listar usuarios (con búsqueda `?q=`) |
| POST | `/api/users` | Crear usuario (participante o super admin) |
| PUT | `/api/users/:id/block` | Bloquear/desbloquear (toggle) |
| DELETE | `/api/users/:id` | Eliminar permanentemente (cascada) |
| POST | `/api/events/:id/assign-admin` | Promover participante a admin de evento |
| POST | `/api/events/:id/remove-admin` | Degradar admin a participante |

### Telegram (requiere JWT)
| Método | Ruta | Descripción |
|---|---|---|
| GET | `/api/events/:id/telegram/group` | Ver si el evento tiene grupo vinculado |
| POST | `/api/events/:id/telegram/group` | Vincular grupo de Telegram al evento |
| POST | `/api/telegram/link-account` | Vincular cuenta personal (comando `/start` del bot) |

---

## 👥 Roles y permisos

```
Super admin
├── Crea eventos, usuarios, bloquea/elimina cuentas
├── Ve todos los eventos y datos de la plataforma
├── Promueve o degrada admins de evento
└── Tiene todos los permisos de admin de evento

Admin de evento
├── Define cronograma (slots, cupos, duración mín/máx)
├── Genera enlace temporal para el cronograma
├── Agrega o retira miembros directamente
├── Aprueba turnos extra, aportes y slots propuestos
├── Define tramo final y rango horario nocturno
├── Ve métricas y ranking de todos los participantes
└── Vincula el grupo de Telegram al evento

Participante
├── Ve solo los eventos donde es miembro activo
├── Se anota en slots del cronograma y ve quién más está anotado
├── Solicita turnos extra espontáneos
├── Hace check-in/out con GPS
├── Registra aportes (pendientes de aprobación)
├── Solicita y confirma reemplazos
└── Ve sus propias métricas y el ranking general (transparencia total)

Invitado (sin cuenta)
└── Se anota en el cronograma vía enlace temporal
    (solo nombre y teléfono — o crea cuenta completa con auto-login)
```

---

## 📊 Las 4 métricas de transparencia

| # | Métrica | Qué incluye | Uso |
|---|---|---|---|
| 1 | **Horas debidas** | Suma de duración de shifts asignados en el cronograma | Referencia |
| 2 | **Horas reales** | Horas efectivas medidas por check-in/out | Verifica el mínimo exigido |
| 3 | **Reales + tramo final** | Métrica 2 + horas del tramo final (si asistió) | Transparencia |
| 4 | **Total con aportes** | Métrica 3 + bono de horas por aportes aprobados | **Orden oficial de la fila** |

> **Regla de habilitación**: si el evento tiene configurado un mínimo de horas, solo pueden aparecer con posición en el ranking quienes hayan cumplido ese mínimo en la métrica 2 (horas reales, sin contar aportes). Esto evita que alguien "compre" su lugar solo aportando cosas sin hacer turnos reales. Quien no cumple aparece sin posición (—).

---

## 🤖 Notificaciones Telegram — Guía completa

Esta sección está pensada para que cualquier admin del campamento pueda configurarlo, sepa Rust o no.

### ¿Qué hace y para qué sirve?

El sistema tiene un bot de Telegram (un usuario automático) que manda avisos solo:

- Al **grupo del campamento**: cuando se libera un turno, cuando hay un enlace nuevo para anotarse, cuando un turno queda sin cubrir
- **En privado a cada persona**: recordatorio 1 hora antes de su turno, cuando le aprueban un aporte o un turno extra, cuando confirman un reemplazo

Es **un solo bot para todos los campamentos** — no hace falta crear un bot nuevo por evento, solo conectar cada grupo de Telegram con su evento correspondiente en la app.

### Cómo interactúa el bot con la app

El bot nunca habla directo con el celular del participante ni con el navegador — todo pasa siempre por el backend. Hay dos caminos, en direcciones opuestas:

**Camino 1 — el sistema avisa a Telegram** (lo más común: un retiro, un aporte aprobado, etc.)
![Flujo saliente de notificaciones](./screenshots/telegram-flujo-saliente.png)
*(diagrama: acción en la app → backend guarda el aviso → worker cada 30s lo recoge → Telegram API → llega al grupo o chat privado)*

**Camino 2 — alguien le pregunta algo al bot** (por ejemplo, el comando `/chatid`)
![Flujo entrante del comando /chatid](./screenshots/telegram-flujo-entrante.png)
*(diagrama: alguien escribe /chatid → Telegram lo guarda esperando → el worker pregunta activamente cada 30s → el backend arma la respuesta → Telegram la entrega)*

> 💡 En ambos casos el worker corre **cada 30 segundos** — por eso los avisos y las respuestas del bot nunca son instantáneos, tardan hasta medio minuto en aparecer. Es normal, no es que algo esté fallando.

### Paso a paso — vincular un grupo nuevo

**1. Crear el grupo de Telegram** del campamento (o usar uno que ya exista).

**2. Agregar el bot al grupo.** Buscá por su nombre de usuario exacto:
```
@campamento_turnos_bot
```
Agregalo como cualquier otro miembro — no hace falta que sea administrador del grupo.

**3. Conseguir el Chat ID del grupo** — es un número (siempre negativo) que identifica a ese grupo específico. La forma más simple:

```
Dentro del grupo, escribí:  /chatid
```

Esperá hasta 30 segundos. El bot responde directamente en el grupo con el número, algo así:

```
🆔 El Chat ID de este grupo ("Campamento Rubén Blades") es:

-1001234567890

Copiá ese número y pegalo en la app, en
Configuración del evento → Notificaciones Telegram → Vincular.
```

**4. Pegar el Chat ID en la app.** Entrá al evento como admin → **Configuración** → tarjeta "Notificaciones Telegram" → pegá el número → **Vincular**.

Listo. Desde ahora, cualquier hueco liberado, enlace nuevo, o aviso grupal llega a ese chat.

### Vincular tu cuenta personal (para recibir avisos privados)

Cada participante que quiera recibir sus propios recordatorios (turno en 1 hora, aporte aprobado, etc.) debe:

```
1. Buscar al bot: @campamento_turnos_bot
2. Abrir un chat privado con él
3. Escribir: /start
```

Con eso queda vinculada su cuenta automáticamente — no necesita pegar ningún número, a diferencia del grupo.

### Troubleshooting — no me llega ningún mensaje

Revisá en este orden:

| Síntoma | Causa probable | Solución |
|---|---|---|
| El bot no responde a `/chatid` | El bot no está en el grupo | Verificá que lo agregaste como miembro |
| El bot no responde a `/chatid` | El backend no está corriendo | Revisá la consola: debe decir "Worker de notificaciones iniciado" |
| Vinculé el grupo pero no llegan avisos | El chat_id se vinculó al evento equivocado | Configuración muestra el badge "Vinculado ✓" — confirmá que estás en el evento correcto |
| Retiré a alguien y no llegó nada | Esa persona no tenía turnos **futuros** | Es esperado: solo se notifica cuando se libera al menos un turno con fecha posterior a ahora |
| Los mensajes tardan | Es normal | El worker revisa cada 30 segundos, no es instantáneo |
| El chat_id dejó de funcionar de golpe | El grupo se convirtió en "supergrupo" | Ver nota abajo — hay que re-vincular con el chat_id nuevo |

### ⚠️ Nota sobre supergrupos

Telegram tiene dos tipos de grupo: "grupo" normal y "supergrupo" (con más funciones, como historial visible para nuevos miembros). Cuando un grupo crece o se activan ciertas configuraciones, Telegram lo convierte automáticamente en supergrupo — **y el Chat ID cambia** (empieza a tener el prefijo `-100`).

Si las notificaciones que venían funcionando dejan de llegar de un día para el otro sin que nadie tocó nada en la app, es la causa más probable. La solución es simple: escribir `/chatid` de nuevo en el grupo y volver a pegar el número actualizado en Configuración.

### Referencia técnica — eventos que disparan notificaciones

| Evento | Destino | Cuándo |
|---|---|---|
| Hueco liberado por retiro | Grupo | Admin retira a un participante con turnos futuros |
| Nuevo enlace de cronograma | Grupo | Admin genera un schedule_link |
| Vacío sin resolver | Grupo | Turno pasa a `gap_unresolved` |
| Turno extra aprobado | Privado | Admin aprueba el turno extra |
| Aporte aprobado | Privado | Admin aprueba la contribución |
| Reemplazo confirmado | Privado (ambos) | `shift_replacement` pasa a confirmed |
| Recordatorio de turno | Privado | 1 hora antes del turno programado |

El worker procesa la cola cada 30 segundos — si Telegram está caído, reintenta en el siguiente ciclo sin bloquear el servidor.

### Referencia técnica — vincular vía API (sin usar la app)

```bash
# Vincular grupo al evento
POST /api/events/:id/telegram/group
{ "telegram_chat_id": "-1001234567890" }

# Ver si un evento ya tiene grupo vinculado
GET /api/events/:id/telegram/group

# Vincular cuenta personal
POST /api/telegram/link-account
{ "telegram_chat_id": "123456789" }
```

---

## 📱 PWA — Instalación en celular

**En producción** (recomendado — HTTPS habilita GPS también en iOS):

1. Abrí `https://appconcert.online` en el celular
2. Android/Chrome: menú (⋮) → "Agregar a pantalla de inicio"
3. iOS/Safari: botón compartir → "Agregar a pantalla de inicio"

**En desarrollo** (misma red WiFi que la PC con Vite):

1. Abrí `http://IP_DEL_SERVIDOR:5174` — mismos pasos de instalación
2. ⚠️ En iOS con HTTP el GPS no funciona (requiere HTTPS) — usar producción para probar GPS en iPhone

---

## 📁 Estructura del proyecto

### Backend (`campamento-api/`)

```
campamento-api/
├── migrations/                  # Migraciones SQL versionadas
├── src/
│   ├── main.rs                  # Punto de entrada, configura servidor y worker
│   ├── config.rs                # Lee variables de entorno
│   ├── db.rs                    # Pool de conexiones a PostgreSQL
│   ├── errors.rs                # Tipos de error y respuestas HTTP
│   ├── auth.rs                  # JWT y hash de contraseñas (Argon2)
│   ├── telegram.rs              # Cliente de la API de Telegram + comando /chatid
│   ├── worker.rs                # Worker de notificaciones (corre cada 30s)
│   ├── models/
│   │   ├── user.rs              # incluye is_blocked
│   │   ├── event.rs
│   │   ├── schedule.rs
│   │   ├── shift.rs
│   │   ├── replacement.rs
│   │   ├── contribution.rs
│   │   └── metrics.rs
│   └── routes/
│       ├── mod.rs               # Router principal + CORS + sirve frontend en prod
│       ├── health.rs
│       ├── auth.rs
│       ├── events.rs            # incluye add_member, filtro por membresía
│       ├── schedule.rs
│       ├── shifts.rs
│       ├── replacements.rs
│       ├── contributions.rs
│       ├── metrics.rs
│       ├── users.rs             # gestión de usuarios, assign/remove admin
│       └── telegram.rs          # link_group, get_group_link
├── Cargo.toml
├── docker-compose.yml
├── .env.example
├── DESPLIEGUE.md                # Guía de despliegue (dev, otra app, Cloudflare)
└── .gitignore
```

### Frontend (`campamento-web/`)

```
campamento-web/
├── public/
│   └── manifest.json
├── src/
│   ├── api/
│   │   ├── client.ts            # URL /api relativa, JWT automático
│   │   ├── auth.ts
│   │   ├── events.ts
│   │   ├── shifts.ts
│   │   └── admin.ts             # eventos, slots, aprobaciones, usuarios, telegram
│   ├── components/
│   │   ├── BottomNav.tsx
│   │   ├── CheckinButton.tsx
│   │   ├── MetricsCard.tsx
│   │   ├── ShiftCard.tsx
│   │   ├── SlotPicker.tsx       # con acordeón de inscriptos
│   │   ├── CreateSlotModal.tsx
│   │   ├── ContributionTypeModal.tsx
│   │   ├── CreateUserModal.tsx
│   │   └── AddMemberModal.tsx
│   ├── hooks/
│   │   └── useAuth.ts
│   ├── pages/
│   │   ├── LoginPage.tsx
│   │   ├── RegisterPage.tsx
│   │   ├── DashboardPage.tsx
│   │   ├── AdminPage.tsx        # ranking + miembros + promover/retirar
│   │   ├── UsersPage.tsx        # gestión de usuarios (super admin)
│   │   ├── RankingPage.tsx      # ranking para participantes
│   │   ├── SettingsPage.tsx     # incluye vinculación Telegram
│   │   └── ScheduleLinkPage.tsx # crea cuenta o invitado, auto-login
│   ├── types/
│   │   └── index.ts
│   ├── main.tsx
│   └── index.css
├── index.html
├── vite.config.ts                # puerto 5174, proxy /api → :8090
├── .env.example
└── .gitignore
```

---

## 🗺️ Próximos pasos

- [ ] Validación de check-in por proximidad al horario del turno
- [ ] Estado "incomplete" si el check-out ocurre antes de un % mínimo del turno
- [ ] Fórmula sugerida ("calcular sugerencia") para el mínimo de horas totales
- [ ] Vista calendario del cronograma (estilo Google Calendar)
- [ ] Subida de foto en check-in (integración con almacenamiento S3-compatible)
- [ ] Adaptación del layout para escritorio (`md:` breakpoints en Tailwind)
- [ ] Integración con WhatsApp Business API (como alternativa a Telegram)
- [ ] Modo offline básico (service worker para ver datos sin conexión)
- [ ] Apps nativas iOS/Android (Capacitor) + landing page
- [ ] Fondo animado Three.js en el login (rama `feat/login-concert-background`)
- [ ] Tests unitarios del backend

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
