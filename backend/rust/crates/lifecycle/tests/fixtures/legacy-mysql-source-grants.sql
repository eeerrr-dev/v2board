CREATE USER 'legacy_reader'@'%'
IDENTIFIED BY 'LegacySourceReadOnlyTestSecret-32-bytes';

GRANT SELECT ON `v2board_legacy`.* TO 'legacy_reader'@'%';
