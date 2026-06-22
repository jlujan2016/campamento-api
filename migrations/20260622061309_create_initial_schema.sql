-- Habilitar extensión UUID (para generar IDs únicos automáticamente)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Habilitar PostGIS (para funciones de geolocalización)
CREATE EXTENSION IF NOT EXISTS postgis;

-- =====================
-- TABLA: users
-- =====================
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email TEXT UNIQUE,                    -- NULL si es usuario invitado
    password_hash TEXT,                   -- NULL si es usuario invitado
    name TEXT NOT NULL,
    phone TEXT,
    is_super_admin BOOLEAN NOT NULL DEFAULT false,
    is_guest BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: events
-- =====================
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    venue_name TEXT NOT NULL,
    lat DOUBLE PRECISION NOT NULL,
    lng DOUBLE PRECISION NOT NULL,
    start_date TIMESTAMPTZ NOT NULL,
    end_date TIMESTAMPTZ NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'finished', 'cancelled')),
    -- Reglas configurables por evento
    min_shift_hours NUMERIC NOT NULL DEFAULT 1,
    max_shift_hours NUMERIC,
    night_start_time TIME,
    night_end_time TIME,
    requires_night_shift BOOLEAN NOT NULL DEFAULT false,
    min_total_hours NUMERIC,
    late_tolerance_minutes NUMERIC NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: event_members
-- =====================
CREATE TABLE event_members (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID NOT NULL REFERENCES events(id),
    user_id UUID NOT NULL REFERENCES users(id),
    role TEXT NOT NULL CHECK (role IN ('admin', 'participant')),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'withdrawn')),
    withdrawn_at TIMESTAMPTZ,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(event_id, user_id)             -- una persona no puede estar dos veces en el mismo evento
);

-- =====================
-- TABLA: schedule_slots
-- =====================
CREATE TABLE schedule_slots (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID NOT NULL REFERENCES events(id),
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ NOT NULL,
    capacity INT NOT NULL DEFAULT 1,
    created_by UUID NOT NULL REFERENCES users(id),
    origin TEXT NOT NULL DEFAULT 'admin'
        CHECK (origin IN ('admin', 'participant')),
    status TEXT NOT NULL DEFAULT 'approved'
        CHECK (status IN ('approved', 'pending_approval')),
    approved_by UUID REFERENCES users(id)
);

-- =====================
-- TABLA: slot_signups
-- =====================
CREATE TABLE slot_signups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    slot_id UUID NOT NULL REFERENCES schedule_slots(id),
    user_id UUID NOT NULL REFERENCES users(id),
    signed_up_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status TEXT NOT NULL DEFAULT 'confirmed'
        CHECK (status IN ('confirmed', 'change_requested', 'cancelled')),
    requested_slot_id UUID REFERENCES schedule_slots(id),
    UNIQUE(slot_id, user_id)
);

-- =====================
-- TABLA: shifts
-- =====================
CREATE TABLE shifts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID NOT NULL REFERENCES events(id),
    user_id UUID NOT NULL REFERENCES users(id),
    shift_type TEXT NOT NULL CHECK (shift_type IN ('scheduled', 'extra')),
    slot_id UUID REFERENCES schedule_slots(id),  -- NULL si es turno extra
    scheduled_start TIMESTAMPTZ NOT NULL,
    scheduled_end TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN (
            'pending', 'approved', 'rejected',
            'active', 'done', 'missed',
            'cancelled', 'gap_unresolved'
        )),
    original_scheduled_start TIMESTAMPTZ,   -- guarda el horario original si se adelantó
    adjustment_reason TEXT,
    approved_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: shift_replacements
-- =====================
CREATE TABLE shift_replacements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    shift_id UUID NOT NULL REFERENCES shifts(id),
    original_user_id UUID NOT NULL REFERENCES users(id),
    replacement_user_id UUID NOT NULL REFERENCES users(id),
    requested_by TEXT NOT NULL CHECK (requested_by IN ('original', 'replacement')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'confirmed', 'rejected')),
    covers_start TIMESTAMPTZ,   -- NULL = reemplazo total del turno
    covers_end TIMESTAMPTZ,     -- NULL = reemplazo total del turno
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ
);

-- =====================
-- TABLA: checkins
-- =====================
CREATE TABLE checkins (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    shift_id UUID NOT NULL REFERENCES shifts(id),
    user_id UUID NOT NULL REFERENCES users(id),
    type TEXT NOT NULL CHECK (type IN ('check_in', 'check_out')),
    lat DOUBLE PRECISION,
    lng DOUBLE PRECISION,
    accuracy_m DOUBLE PRECISION,
    photo_url TEXT,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: final_checkpoints
-- =====================
CREATE TABLE final_checkpoints (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID NOT NULL REFERENCES events(id),
    opens_at TIMESTAMPTZ NOT NULL,
    description TEXT
);

-- =====================
-- TABLA: final_attendance
-- =====================
CREATE TABLE final_attendance (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    final_checkpoint_id UUID NOT NULL REFERENCES final_checkpoints(id),
    user_id UUID NOT NULL REFERENCES users(id),
    checkin_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lat DOUBLE PRECISION,
    lng DOUBLE PRECISION,
    status TEXT NOT NULL DEFAULT 'present'
        CHECK (status IN ('present', 'absent')),
    UNIQUE(final_checkpoint_id, user_id)
);

-- =====================
-- TABLA: contribution_types
-- =====================
CREATE TABLE contribution_types (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID NOT NULL REFERENCES events(id),
    type_key TEXT NOT NULL,           -- ej. 'tent', 'mattress', 'food'
    label TEXT NOT NULL,              -- nombre visible, ej. "Carpa"
    default_hour_bonus NUMERIC NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: contributions
-- =====================
CREATE TABLE contributions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID NOT NULL REFERENCES events(id),
    user_id UUID NOT NULL REFERENCES users(id),
    contribution_type_id UUID NOT NULL REFERENCES contribution_types(id),
    description TEXT,
    hour_bonus NUMERIC NOT NULL,
    approved_by UUID REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: schedule_links
-- =====================
CREATE TABLE schedule_links (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID NOT NULL REFERENCES events(id),
    token TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: telegram_links
-- =====================
CREATE TABLE telegram_links (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID REFERENCES users(id),
    telegram_chat_id TEXT NOT NULL,
    event_id UUID REFERENCES events(id),
    link_type TEXT NOT NULL CHECK (link_type IN ('group', 'private')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =====================
-- TABLA: notifications
-- =====================
CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id UUID REFERENCES events(id),
    user_id UUID REFERENCES users(id),
    type TEXT NOT NULL CHECK (type IN (
        'slot_freed', 'new_schedule_link', 'extra_approved',
        'contribution_approved', 'replacement_confirmed',
        'shift_reminder', 'gap_unresolved'
    )),
    payload JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'sent', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sent_at TIMESTAMPTZ
);-- Add migration script here
