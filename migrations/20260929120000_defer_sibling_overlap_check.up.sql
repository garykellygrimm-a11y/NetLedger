ALTER TABLE subnet DROP CONSTRAINT subnet_no_overlapping_siblings;

ALTER TABLE subnet
    ADD CONSTRAINT subnet_no_overlapping_siblings
    EXCLUDE USING gist (
        (COALESCE(parent_id, '00000000-0000-0000-0000-000000000000'::uuid)) WITH =,
        cidr inet_ops WITH &&
    )
    DEFERRABLE INITIALLY IMMEDIATE;
