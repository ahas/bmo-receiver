-- The supervisors a room has turned off: each still stands in the room, named or by its account, and answers
-- nothing there while it stands here (`docs/wake.md`, 「Who reaches the session」).

CREATE TABLE room_supervisors_off (
    chat_id bigint NOT NULL,
    supervisor text NOT NULL,
    PRIMARY KEY (chat_id, supervisor)
);
