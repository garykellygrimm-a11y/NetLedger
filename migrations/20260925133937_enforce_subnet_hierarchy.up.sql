CREATE EXTENSION IF NOT EXISTS btree_gist;

DO $$
DECLARE
    bad record;
BEGIN
    SELECT child.cidr AS child_cidr, parent.cidr AS parent_cidr
    INTO bad
    FROM subnet child
    JOIN subnet parent ON parent.id = child.parent_id
    WHERE NOT child.cidr << parent.cidr
    LIMIT 1;

    IF FOUND THEN
        RAISE EXCEPTION 'subnet % is not inside its parent %; correct its parent before upgrading',
            bad.child_cidr, bad.parent_cidr;
    END IF;

    SELECT a.cidr AS first_cidr, b.cidr AS second_cidr
    INTO bad
    FROM subnet a
    JOIN subnet b
        ON a.id < b.id
        AND a.parent_id IS NOT DISTINCT FROM b.parent_id
        AND a.cidr && b.cidr
    LIMIT 1;

    IF FOUND THEN
        RAISE EXCEPTION 'subnets % and % overlap at the same level; make one the parent of the other before upgrading',
            bad.first_cidr, bad.second_cidr;
    END IF;
END
$$;

ALTER TABLE subnet
    ADD CONSTRAINT subnet_no_overlapping_siblings
    EXCLUDE USING gist (
        (COALESCE(parent_id, '00000000-0000-0000-0000-000000000000'::uuid)) WITH =,
        cidr inet_ops WITH &&
    );

CREATE FUNCTION subnet_check_within_parent() RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    parent_cidr cidr;
BEGIN
    IF NEW.parent_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT cidr INTO parent_cidr
    FROM subnet
    WHERE id = NEW.parent_id
    FOR SHARE;

    IF FOUND AND NOT NEW.cidr << parent_cidr THEN
        RAISE EXCEPTION 'subnet % is not inside its parent %', NEW.cidr, parent_cidr
            USING ERRCODE = 'check_violation', CONSTRAINT = 'subnet_within_parent';
    END IF;

    RETURN NEW;
END
$$;

CREATE TRIGGER subnet_within_parent
    BEFORE INSERT OR UPDATE OF cidr, parent_id ON subnet
    FOR EACH ROW
    EXECUTE FUNCTION subnet_check_within_parent();
