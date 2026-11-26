-- The agents standing in a room read off an index of the room's own lines, so telling who answers there is a walk of
-- the agents' rows alone and never of the room's whole record (`docs/api.md`, 「Rooms」).

CREATE INDEX messages_room_agent ON messages USING btree (chat_id, agent_name) WHERE (agent_name IS NOT NULL);
