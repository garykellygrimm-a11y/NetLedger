CREATE TABLE address (
    id          uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    address     inet        NOT NULL UNIQUE,
    subnet_id   uuid        NOT NULL REFERENCES subnet (id) ON DELETE RESTRICT,
    hostname    text        NOT NULL DEFAULT '',
    description text        NOT NULL DEFAULT '',
    source      text        NOT NULL CHECK (source IN ('manual', 'allocated', 'discovered')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT address_is_single_host
        CHECK (masklen(address) = CASE family(address) WHEN 4 THEN 32 ELSE 128 END)
);

CREATE INDEX address_subnet_id_idx ON address (subnet_id);

CREATE FUNCTION address_check_within_subnet() RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    subnet_cidr cidr;
BEGIN
    SELECT cidr INTO subnet_cidr
    FROM subnet
    WHERE id = NEW.subnet_id
    FOR SHARE;

    IF FOUND AND NOT NEW.address <<= subnet_cidr THEN
        RAISE EXCEPTION 'address % is not inside subnet %', NEW.address, subnet_cidr
            USING ERRCODE = 'check_violation', CONSTRAINT = 'address_within_subnet';
    END IF;

    RETURN NEW;
END
$$;

CREATE TRIGGER address_within_subnet
    BEFORE INSERT OR UPDATE OF address, subnet_id ON address
    FOR EACH ROW
    EXECUTE FUNCTION address_check_within_subnet();
