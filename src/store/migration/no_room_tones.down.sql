CREATE TABLE room_tones (
    chat_id bigint NOT NULL,
    tone text NOT NULL,
    set_at bigint NOT NULL,
    rowid bigint GENERATED ALWAYS AS (chat_id) STORED
);

ALTER TABLE ONLY room_tones
    ADD CONSTRAINT room_tones_pkey PRIMARY KEY (chat_id);
