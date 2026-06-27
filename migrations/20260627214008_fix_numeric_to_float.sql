-- Cambiamos todos los campos NUMERIC a FLOAT8
-- FLOAT8 en Postgres = f64 en Rust, compatible directamente con SQLx sin features extra

ALTER TABLE events
    ALTER COLUMN min_shift_hours TYPE FLOAT8,
    ALTER COLUMN max_shift_hours TYPE FLOAT8,
    ALTER COLUMN min_total_hours TYPE FLOAT8,
    ALTER COLUMN late_tolerance_minutes TYPE FLOAT8;

ALTER TABLE shifts
    ALTER COLUMN scheduled_start TYPE TIMESTAMPTZ,
    ALTER COLUMN scheduled_end TYPE TIMESTAMPTZ;

ALTER TABLE contribution_types
    ALTER COLUMN default_hour_bonus TYPE FLOAT8;

ALTER TABLE contributions
    ALTER COLUMN hour_bonus TYPE FLOAT8;
