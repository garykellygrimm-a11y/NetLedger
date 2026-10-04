DROP TRIGGER audit_log_no_truncate ON audit_log;
DROP TRIGGER audit_log_append_only ON audit_log;
DROP FUNCTION audit_log_is_append_only();
DROP TABLE audit_log;
DROP TABLE api_token;
DROP TABLE session;
DROP TABLE identity;
DROP TABLE account;
