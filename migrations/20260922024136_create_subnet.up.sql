CREATE TABLE subnet (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    cidr        cidr        NOT NULL UNIQUE,
    name        text        NOT NULL,
    description text        NOT NULL DEFAULT '',
    vlan_id     integer     CHECK (vlan_id BETWEEN 1 AND 4094),
    parent_id   uuid        REFERENCES subnet (id) ON DELETE RESTRICT,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX subnet_parent_id_idx ON subnet (parent_id);
