-- Group item payload delivery (sharing-pki.md §6).
--
-- 0003_sharing.sql created `group_items` to record the group↔item association
-- but stored no ciphertext. Group sharing encrypts the item payload once under
-- the unified Group SIK; the admin delivers that ciphertext here and every
-- member decrypts it with the Group SIK they hold via their wrapped key
-- (§6.1-6.2). This migration adds the payload column.

ALTER TABLE group_items ADD COLUMN payload BLOB;
