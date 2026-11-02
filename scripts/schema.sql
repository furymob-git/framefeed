create table if not exists videos (
  slug text primary key,
  title text not null,
  description text not null default '',
  stream_url text not null,
  width_px integer not null check (width_px > 0),
  height_px integer not null check (height_px > 0 and height_px % 2 = 0),
  cols integer not null,
  rows integer not null,
  fps integer not null check (fps between 1 and 60),
  frame_count integer not null,
  duration_seconds double precision not null,
  size_bytes bigint not null,
  format text not null default 'ansi',
  published boolean not null default true,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  constraint slug_format check (slug ~ '^[a-z0-9][a-z0-9-]{0,62}$')
);
create index if not exists videos_published_created_idx on videos (published, created_at desc);
alter table videos add column if not exists format text not null default 'ansi';