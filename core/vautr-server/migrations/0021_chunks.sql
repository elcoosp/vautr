-- Vautr server schema — encrypted file chunks (FEAT-H01). Append-only.
CREATE TABLE IF NOT EXISTS chunks (
    file_uuid TEXT NOT NULL,
    idx       INTEGER NOT NULL,
    bytes     BLOB NOT NULL,
    PRIMARY KEY (file_uuid, idx)
) STRICT;
