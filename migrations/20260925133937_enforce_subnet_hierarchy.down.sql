DROP TRIGGER subnet_within_parent ON subnet;
DROP FUNCTION subnet_check_within_parent();
ALTER TABLE subnet DROP CONSTRAINT subnet_no_overlapping_siblings;
