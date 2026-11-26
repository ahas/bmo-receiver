-- The mark of the addition that set a room's list of providers: a room added on a chat service keeps that service,
-- while a list set on a room never added is its providers being chosen and stays open to be set again
-- (`docs/receiver.md`, 「A room's chat providers」). A room already standing in a mode had its list fixed, so its
-- list takes the mark.

ALTER TABLE ONLY room_providers
    ADD COLUMN added boolean NOT NULL DEFAULT false;

UPDATE room_providers AS held
    SET added = true
    FROM room_modes AS mode
    WHERE mode.chat_id = held.chat_id;
