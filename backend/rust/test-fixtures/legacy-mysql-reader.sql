CREATE USER 'v2board_reader'@'%' IDENTIFIED BY 'v2board-reader-test-password';
GRANT SELECT, SHOW VIEW ON `v2board`.* TO 'v2board_reader'@'%';
GRANT SHOW DATABASES, PROCESS, REPLICATION CLIENT ON *.* TO 'v2board_reader'@'%';
GRANT SELECT ON `performance_schema`.`replication_connection_status` TO 'v2board_reader'@'%';
GRANT SELECT ON `performance_schema`.`replication_group_members` TO 'v2board_reader'@'%';

CREATE USER 'v2board_fence'@'%' IDENTIFIED BY 'v2board-fence-test-password';
GRANT PROCESS, SYSTEM_VARIABLES_ADMIN ON *.* TO 'v2board_fence'@'%';
